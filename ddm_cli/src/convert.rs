use std::{
    fmt::Display,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
    sync::mpsc,
    thread,
};

use ddm_app::{DDMFilters, L0bFormatConfig, fanout};
use ddm_core::l0b::{
    DDMFrame, L0bPacket, MP00Encoder, MP00EncoderConfig, MQ00Encoder, MQ00EncoderConfig,
};
use prodash::{
    NestedProgress, Progress as _,
    progress::Discard,
    unit::{self, Bytes, Human, display::Mode},
};

use crate::{HEIGHT, WIDTH, table::format_file_info};

#[expect(clippy::too_many_lines, reason = "Pipeline definition")]
pub fn convert<P>(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    filters: &DDMFilters,
    format: &L0bFormatConfig,
    progress: &mut P,
) -> anyhow::Result<impl Display>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    let file_name = input
        .as_ref()
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or(input.as_ref().to_string_lossy())
        .into_owned();
    let output_name = output
        .as_ref()
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or(input.as_ref().to_string_lossy())
        .into_owned();
    let mut file_progress = progress.add_child(format!("Convert {file_name} into {output_name}"));
    let mut read_bar = file_progress.add_child("Read packets");
    let mut decode_bar = file_progress.add_child("Decode frames");
    let mut filter_bar = file_progress.add_child("Filter frames");
    let mut encode_bar = file_progress.add_child("Encode frames");
    let mut write_bar = file_progress.add_child("Write packets");
    #[expect(clippy::cast_possible_truncation, reason = "Non essential")]
    let len = std::fs::metadata(input.as_ref())?.len() as usize;
    read_bar.init(
        Some(len),
        Some(unit::dynamic_and_mode(
            Bytes,
            Mode::with_throughput().and_percentage(),
        )),
    );
    let mut formatter = human_format::Formatter::new();
    formatter.with_decimals(2);
    decode_bar.init(
        None,
        Some(unit::dynamic_and_mode(
            Human::new(formatter, "frames"),
            Mode::with_throughput().and_percentage(),
        )),
    );
    let mut formatter = human_format::Formatter::new();
    formatter.with_decimals(2);
    filter_bar.init(
        None,
        Some(unit::dynamic_and_mode(
            Human::new(formatter, "frames"),
            Mode::with_throughput().and_percentage(),
        )),
    );
    let mut formatter = human_format::Formatter::new();
    formatter.with_decimals(2);
    encode_bar.init(
        None,
        Some(unit::dynamic_and_mode(
            Human::new(formatter, "frames"),
            Mode::with_throughput().and_percentage(),
        )),
    );
    let mut formatter = human_format::Formatter::new();
    formatter.with_decimals(2);
    write_bar.init(
        None,
        Some(unit::dynamic_and_mode(
            Human::new(formatter, "packets"),
            Mode::with_throughput().and_percentage(),
        )),
    );

    let mut read_packet_stream: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut read_packet_stream_inspect: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut read_packet_stream_decode: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut filter_frame_stream: (mpsc::SyncSender<DDMFrame<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut encode_frame_stream: (mpsc::SyncSender<DDMFrame<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut write_packet_stream: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut write_packet_stream_inspect: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut write_packet_stream_write: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut reader = BufReader::new(File::open(input.as_ref())?);
    let mut writer = BufWriter::new(File::create(output.as_ref())?);
    let read_thread =
        thread::spawn(move || ddm_app::read(&mut reader, &mut read_packet_stream.0, &mut read_bar));
    let fanout_original_thread = thread::spawn(move || {
        fanout(
            &mut read_packet_stream.1,
            &mut [read_packet_stream_inspect.0, read_packet_stream_decode.0],
        )
    });
    let inspect_original_thread =
        thread::spawn(move || ddm_app::inspect(&mut read_packet_stream_inspect.1, &mut Discard));
    let decode_thread = thread::spawn(move || {
        ddm_app::decode(
            &mut read_packet_stream_decode.1,
            &mut filter_frame_stream.0,
            &mut decode_bar,
        )
    });
    let filters = filters.clone();
    let filter_thread = thread::spawn(move || {
        ddm_app::filter(
            &mut filter_frame_stream.1,
            &mut encode_frame_stream.0,
            &filters,
            &mut filter_bar,
        )
    });
    let encode_thread = match format {
        L0bFormatConfig::MP00 { quantisation } => {
            let config = MP00EncoderConfig::new(*quantisation);
            let mut encoder = MP00Encoder::new(config);
            thread::spawn(move || {
                ddm_app::encode(
                    &mut encode_frame_stream.1,
                    &mut write_packet_stream.0,
                    &mut encoder,
                    &mut encode_bar,
                )
            })
        }
        L0bFormatConfig::MQ00 {
            quantisation,
            dc_adjust,
            wavelet_stages,
            gop_length,
        } => {
            let config =
                MQ00EncoderConfig::new(*quantisation, *dc_adjust, *wavelet_stages, *gop_length)?;
            let mut encoder = MQ00Encoder::new(config);
            thread::spawn(move || {
                ddm_app::encode(
                    &mut encode_frame_stream.1,
                    &mut write_packet_stream.0,
                    &mut encoder,
                    &mut encode_bar,
                )
            })
        }
    };
    let fanout_converted_thread = thread::spawn(move || {
        fanout(
            &mut write_packet_stream.1,
            &mut [write_packet_stream_inspect.0, write_packet_stream_write.0],
        )
    });
    let inspect_converted_thread =
        thread::spawn(move || ddm_app::inspect(&mut write_packet_stream_inspect.1, &mut Discard));
    let write_thread = thread::spawn(move || {
        ddm_app::write(
            &mut writer,
            &mut write_packet_stream_write.1,
            &mut write_bar,
        )
    });

    read_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Read thread panicked"))??;
    decode_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Decode thread panicked"))??;
    filter_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Filter thread panicked"))??;
    encode_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Encode thread panicked"))??;
    write_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Write thread panicked"))??;
    fanout_original_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Fanout original thread panicked"))??;
    fanout_converted_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Fanout converted thread panicked"))??;

    let original = inspect_original_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Inspect original thread panicked"))?;

    let converted = inspect_converted_thread
        .join()
        .map_err(|_| anyhow::anyhow!("Inspect original thread panicked"))?;

    file_progress.done("Done".to_owned());
    let file_info = [
        (format!("Original ({file_name})"), original),
        (format!("Converted ({output_name})"), converted),
    ];
    Ok(format_file_info(&file_info))
}
