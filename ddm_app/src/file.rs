use std::{
    io::{Read, Seek, SeekFrom, Write},
    thread::{self, JoinHandle},
};

use ddm_core::l0b::{
    DDMFrame, L0bPacket, MP00Encoder, MP00EncoderConfig, MQ00Encoder, MQ00EncoderConfig,
};
use prodash::{
    NestedProgress, Progress,
    progress::Discard,
    unit::{self, Bytes, Human, display::Mode},
};

use crate::{DDMFilters, DDMFrameMap, L0bFileInfo, L0bFormatConfig, PipelineChannel, fanout};

const PIPELINE_CAPACITY: usize = 100;

/// An L0b file's metadata and decoded frame map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L0bFileData<const WIDTH: usize, const HEIGHT: usize> {
    /// Metadata collected from the file's packets.
    pub info: L0bFileInfo,
    /// Decoded frames indexed by channel and GPS timestamp.
    pub frames: DDMFrameMap<WIDTH, HEIGHT>,
}

/// Metadata collected while converting one L0b file to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L0bConversionResult {
    /// Metadata from the source file.
    pub source: L0bFileInfo,
    /// Metadata from the destination file.
    pub destination: L0bFileInfo,
}

struct LoadL0bTasks<const WIDTH: usize, const HEIGHT: usize> {
    read: JoinHandle<anyhow::Result<()>>,
    fanout: JoinHandle<anyhow::Result<()>>,
    decode: JoinHandle<anyhow::Result<()>>,
    inspect: JoinHandle<L0bFileInfo>,
    gather: JoinHandle<DDMFrameMap<WIDTH, HEIGHT>>,
}

enum EncoderConfig {
    Mp00(u8),
    Mq00(MQ00EncoderConfig),
}

impl<const WIDTH: usize, const HEIGHT: usize> LoadL0bTasks<WIDTH, HEIGHT> {
    fn join(self) -> anyhow::Result<L0bFileData<WIDTH, HEIGHT>> {
        self.read
            .join()
            .map_err(|_| anyhow::anyhow!("Read thread panicked"))??;
        self.fanout
            .join()
            .map_err(|_| anyhow::anyhow!("Fanout thread panicked"))??;
        self.decode
            .join()
            .map_err(|_| anyhow::anyhow!("Decode thread panicked"))??;
        let info = self
            .inspect
            .join()
            .map_err(|_| anyhow::anyhow!("Inspect thread panicked"))?;
        let frames = self
            .gather
            .join()
            .map_err(|_| anyhow::anyhow!("Gather thread panicked"))?;
        Ok(L0bFileData { info, frames })
    }
}

/// Inspect an L0b file's packet metadata.
///
/// # Errors
/// Returns errors from the input reader, packet reading, or worker threads.
pub fn inspect_l0b<const WIDTH: usize, const HEIGHT: usize, P>(
    reader: impl Read + Seek + Send + 'static,
    progress: &mut P,
) -> anyhow::Result<L0bFileInfo>
where
    P: NestedProgress,
    P::SubProgress: Send + 'static,
{
    let mut read_progress = progress.add_child("Read packets");
    let mut inspect_progress = progress.add_child("Inspect metadata");
    let mut reader = reader;
    init_read_progress(&mut read_progress, &mut reader)?;
    init_count_progress(&mut inspect_progress, "packets");

    let mut packets: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let read =
        thread::spawn(move || crate::read(&mut reader, &mut packets.sender, &mut read_progress));
    let inspect =
        thread::spawn(move || crate::inspect(&mut packets.receiver, &mut inspect_progress));

    read.join()
        .map_err(|_| anyhow::anyhow!("Read thread panicked"))??;
    let info = inspect
        .join()
        .map_err(|_| anyhow::anyhow!("Inspect thread panicked"))?;
    progress.done("Done".to_owned());
    Ok(info)
}

/// Read, inspect, and decode an L0b file into a frame map.
///
/// # Errors
/// Returns errors from the input reader, packet reading, decoding, or worker threads.
pub fn load_l0b<const WIDTH: usize, const HEIGHT: usize, P>(
    reader: impl Read + Seek + Send + 'static,
    progress: &mut P,
) -> anyhow::Result<L0bFileData<WIDTH, HEIGHT>>
where
    P: NestedProgress,
    P::SubProgress: Send + 'static,
{
    let mut file_progress = progress.add_child("Load and map L0b");
    let mut read_progress = file_progress.add_child("Read packets");
    let mut decode_progress = file_progress.add_child("Decode frames");
    let mut gather_progress = file_progress.add_child("Gather frames");
    let mut reader = reader;
    init_read_progress(&mut read_progress, &mut reader)?;
    init_count_progress(&mut decode_progress, "frames");
    init_count_progress(&mut gather_progress, "frames");

    let mut read_stream: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut inspect_stream: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut decode_stream: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut frame_stream: PipelineChannel<DDMFrame<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);

    let read = thread::spawn(move || {
        crate::read(&mut reader, &mut read_stream.sender, &mut read_progress)
    });
    let fanout = thread::spawn(move || {
        fanout(
            &mut read_stream.receiver,
            &mut [inspect_stream.sender, decode_stream.sender],
        )
    });
    let inspect = thread::spawn(move || crate::inspect(&mut inspect_stream.receiver, &mut Discard));
    let decode = thread::spawn(move || {
        crate::decode(
            &mut decode_stream.receiver,
            &mut frame_stream.sender,
            &mut decode_progress,
        )
    });
    let gather = thread::spawn(move || {
        DDMFrameMap::gather(&mut frame_stream.receiver, &mut gather_progress)
    });

    let loaded = LoadL0bTasks {
        read,
        fanout,
        decode,
        inspect,
        gather,
    }
    .join()?;
    file_progress.done("Done".to_owned());
    Ok(loaded)
}

/// Convert an L0b file by filtering its frames and encoding them in another format.
///
/// # Errors
/// Returns errors from either stream, encoder configuration, pipeline stages, or worker threads.
#[expect(
    clippy::too_many_lines,
    reason = "The pipeline stages are kept together for clarity"
)]
pub fn convert_l0b<const WIDTH: usize, const HEIGHT: usize, P>(
    reader: impl Read + Seek + Send + 'static,
    writer: impl Write + Seek + Send + 'static,
    filters: &DDMFilters,
    format: &L0bFormatConfig,
    progress: &mut P,
) -> anyhow::Result<L0bConversionResult>
where
    P: NestedProgress,
    P::SubProgress: Send + 'static,
{
    let mut file_progress = progress.add_child("Convert L0b file");
    let mut read_progress = file_progress.add_child("Read packets");
    let mut decode_progress = file_progress.add_child("Decode frames");
    let mut filter_progress = file_progress.add_child("Filter frames");
    let mut encode_progress = file_progress.add_child("Encode frames");
    let mut write_progress = file_progress.add_child("Write packets");
    let mut reader = reader;
    let mut writer = writer;
    init_read_progress(&mut read_progress, &mut reader)?;
    init_count_progress(&mut decode_progress, "frames");
    init_count_progress(&mut filter_progress, "frames");
    init_count_progress(&mut encode_progress, "frames");
    init_count_progress(&mut write_progress, "packets");

    // Validate encoder settings before starting workers, so invalid options cannot strand a
    // partially started pipeline.
    let encoder_config = match format {
        L0bFormatConfig::MP00 { quantisation } => EncoderConfig::Mp00(*quantisation),
        L0bFormatConfig::MQ00 {
            quantisation,
            dc_adjust,
            wavelet_stages,
            gop_length,
        } => EncoderConfig::Mq00(MQ00EncoderConfig::new(
            *quantisation,
            *dc_adjust,
            *wavelet_stages,
            *gop_length,
        )?),
    };

    let mut read_packet_stream: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut read_packet_stream_inspect: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut read_packet_stream_decode: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut filter_frame_stream: PipelineChannel<DDMFrame<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut encode_frame_stream: PipelineChannel<DDMFrame<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut write_packet_stream: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut write_packet_stream_inspect: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);
    let mut write_packet_stream_write: PipelineChannel<L0bPacket<WIDTH, HEIGHT>> =
        PipelineChannel::bounded(PIPELINE_CAPACITY);

    let read = thread::spawn(move || {
        crate::read(
            &mut reader,
            &mut read_packet_stream.sender,
            &mut read_progress,
        )
    });
    let fanout_original = thread::spawn(move || {
        fanout(
            &mut read_packet_stream.receiver,
            &mut [
                read_packet_stream_inspect.sender,
                read_packet_stream_decode.sender,
            ],
        )
    });
    let inspect_original = thread::spawn(move || {
        crate::inspect(&mut read_packet_stream_inspect.receiver, &mut Discard)
    });
    let decode = thread::spawn(move || {
        crate::decode(
            &mut read_packet_stream_decode.receiver,
            &mut filter_frame_stream.sender,
            &mut decode_progress,
        )
    });
    let filters = filters.clone();
    let filter = thread::spawn(move || {
        crate::filter(
            &mut filter_frame_stream.receiver,
            &mut encode_frame_stream.sender,
            &filters,
            &mut filter_progress,
        )
    });
    let encode = match encoder_config {
        EncoderConfig::Mp00(quantisation) => {
            let mut encoder = MP00Encoder::new(MP00EncoderConfig::new(quantisation));
            thread::spawn(move || {
                crate::encode(
                    &mut encode_frame_stream.receiver,
                    &mut write_packet_stream.sender,
                    &mut encoder,
                    &mut encode_progress,
                )
            })
        }
        EncoderConfig::Mq00(config) => {
            let mut encoder = MQ00Encoder::new(config);
            thread::spawn(move || {
                crate::encode(
                    &mut encode_frame_stream.receiver,
                    &mut write_packet_stream.sender,
                    &mut encoder,
                    &mut encode_progress,
                )
            })
        }
    };
    let fanout_converted = thread::spawn(move || {
        fanout(
            &mut write_packet_stream.receiver,
            &mut [
                write_packet_stream_inspect.sender,
                write_packet_stream_write.sender,
            ],
        )
    });
    let inspect_converted = thread::spawn(move || {
        crate::inspect(&mut write_packet_stream_inspect.receiver, &mut Discard)
    });
    let write = thread::spawn(move || {
        crate::write(
            &mut writer,
            &mut write_packet_stream_write.receiver,
            &mut write_progress,
        )
    });

    read.join()
        .map_err(|_| anyhow::anyhow!("Read thread panicked"))??;
    decode
        .join()
        .map_err(|_| anyhow::anyhow!("Decode thread panicked"))??;
    filter
        .join()
        .map_err(|_| anyhow::anyhow!("Filter thread panicked"))??;
    encode
        .join()
        .map_err(|_| anyhow::anyhow!("Encode thread panicked"))??;
    write
        .join()
        .map_err(|_| anyhow::anyhow!("Write thread panicked"))??;
    fanout_original
        .join()
        .map_err(|_| anyhow::anyhow!("Original fanout thread panicked"))??;
    fanout_converted
        .join()
        .map_err(|_| anyhow::anyhow!("Converted fanout thread panicked"))??;
    let source = inspect_original
        .join()
        .map_err(|_| anyhow::anyhow!("Source inspection thread panicked"))?;
    let destination = inspect_converted
        .join()
        .map_err(|_| anyhow::anyhow!("Destination inspection thread panicked"))?;

    file_progress.done("Done".to_owned());
    Ok(L0bConversionResult {
        source,
        destination,
    })
}

fn init_read_progress<P, R>(progress: &mut P, reader: &mut R) -> anyhow::Result<()>
where
    P: Progress,
    R: Seek,
{
    let position = reader.stream_position()?;
    let len = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(position))?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Progress display is nonessential"
    )]
    let len = len as usize;
    progress.init(
        Some(len),
        Some(unit::dynamic_and_mode(
            Bytes,
            Mode::with_throughput().and_percentage(),
        )),
    );
    Ok(())
}

fn init_count_progress<P: Progress>(progress: &mut P, unit_name: &'static str) {
    let mut formatter = human_format::Formatter::new();
    formatter.with_decimals(2);
    progress.init(
        None,
        Some(unit::dynamic_and_mode(
            Human::new(formatter, unit_name),
            Mode::with_throughput().and_percentage(),
        )),
    );
}
