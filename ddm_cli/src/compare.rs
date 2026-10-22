use std::{
    fmt::Display,
    fs::File,
    io::BufReader,
    path::Path,
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
};

use ddm_app::{DDMFrameMap, L0bFileInfo, fanout};
use ddm_core::l0b::{DDMFrame, L0bPacket};
use prodash::{
    NestedProgress, Progress,
    progress::Discard,
    unit::{self, Bytes, Human, display::Mode},
};
use tracing::debug;

use crate::{HEIGHT, WIDTH, table::format_comparison_info};

struct LoadL0bJoinHandles {
    read_thread: JoinHandle<anyhow::Result<()>>,
    fanout_thread: JoinHandle<anyhow::Result<()>>,
    decode_thread: JoinHandle<anyhow::Result<()>>,
    inspect_thread: JoinHandle<L0bFileInfo>,
    gather_thread: JoinHandle<DDMFrameMap<WIDTH, HEIGHT>>,
}

impl LoadL0bJoinHandles {
    pub fn join(self) -> anyhow::Result<(L0bFileInfo, DDMFrameMap<WIDTH, HEIGHT>)> {
        self.read_thread
            .join()
            .map_err(|_| anyhow::anyhow!("Read thread panicked"))??;
        self.fanout_thread
            .join()
            .map_err(|_| anyhow::anyhow!("Read thread panicked"))??;
        self.decode_thread
            .join()
            .map_err(|_| anyhow::anyhow!("Read thread panicked"))??;
        let file_info = self
            .inspect_thread
            .join()
            .map_err(|_| anyhow::anyhow!("Read thread panicked"))?;
        let frame_map = self
            .gather_thread
            .join()
            .map_err(|_| anyhow::anyhow!("Read thread panicked"))?;
        Ok((file_info, frame_map))
    }
}

fn load_l0b_to_map<P>(
    file: impl AsRef<Path>,
    progress: &mut P,
) -> anyhow::Result<LoadL0bJoinHandles>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    let file_name = file
        .as_ref()
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or(file.as_ref().to_string_lossy())
        .into_owned();
    let mut file_progress = progress.add_child(format!("Load and map {file_name}"));
    let mut read_bar = file_progress.add_child("Read packets");
    let mut decode_bar = file_progress.add_child("Decode frames");
    let mut gather_bar = file_progress.add_child("Gather frames");
    #[expect(clippy::cast_possible_truncation, reason = "Non essential")]
    let len = std::fs::metadata(file.as_ref())?.len() as usize;
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
    gather_bar.init(
        None,
        Some(unit::dynamic_and_mode(
            Human::new(formatter, "frames"),
            Mode::with_throughput().and_percentage(),
        )),
    );

    let mut read_packet_stream: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut read_packet_stream_inspect: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut read_packet_stream_decode: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);
    let mut gather_frame_stream: (mpsc::SyncSender<DDMFrame<WIDTH, HEIGHT>>, _) =
        mpsc::sync_channel(100);

    let mut reader = BufReader::new(File::open(file.as_ref())?);

    let read_thread =
        thread::spawn(move || ddm_app::read(&mut reader, &mut read_packet_stream.0, &mut read_bar));
    let fanout_thread = thread::spawn(move || {
        fanout(
            &mut read_packet_stream.1,
            &mut [read_packet_stream_inspect.0, read_packet_stream_decode.0],
        )
    });
    let inspect_thread =
        thread::spawn(move || ddm_app::inspect(&mut read_packet_stream_inspect.1, &mut Discard));
    let decode_thread = thread::spawn(move || {
        ddm_app::decode(
            &mut read_packet_stream_decode.1,
            &mut gather_frame_stream.0,
            &mut decode_bar,
        )
    });
    let gather_thread =
        thread::spawn(move || DDMFrameMap::gather(&mut gather_frame_stream.1, &mut gather_bar));
    Ok(LoadL0bJoinHandles {
        read_thread,
        fanout_thread,
        decode_thread,
        inspect_thread,
        gather_thread,
    })
}

pub fn compare<I: IntoIterator<Item = impl AsRef<Path>>, P>(
    files: I,
    progress: &mut P,
) -> anyhow::Result<impl Display>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    type LoadResult<const W: usize, const H: usize> =
        anyhow::Result<(L0bFileInfo, DDMFrameMap<W, H>)>;

    let (load_tx, load_rx) = mpsc::channel::<(usize, String, LoadResult<WIDTH, HEIGHT>)>();

    let mut load_threads = Vec::new();
    let mut reference_load = None;

    for (index, file) in files.into_iter().enumerate() {
        let file_name = file
            .as_ref()
            .file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or(file.as_ref().to_string_lossy())
            .into_owned();

        let mut file_progress = progress.add_child(file_name.clone());
        let handles = load_l0b_to_map(file, &mut file_progress)?;

        if index == 0 {
            // The first file is the reference.
            reference_load = Some((file_name, handles));
        } else {
            // Wait for this file to finish loading, then notify the receiver.
            let tx = load_tx.clone();

            load_threads.push(thread::spawn(move || {
                let result = handles.join();
                debug!(
                    thread_id = ?thread_id::get(),
                    "finished loading file {file_name}"
                );
                let _ = tx.send((index, file_name, result));
            }));
        }
    }

    // Allow the receiver to observe when all senders have gone away.
    drop(load_tx);

    let Some((reference_name, reference_handles)) = reference_load else {
        anyhow::bail!("At least one input file is required");
    };

    let (reference_info, reference_frame_map) = reference_handles.join()?;
    let reference_map = Arc::new(reference_frame_map);

    let mut compare_threads = Vec::with_capacity(load_threads.len() + 1);
    compare_threads.push((0, reference_name, reference_info, None));

    // Each receive blocks until another file has finished loading.
    for _ in 0..load_threads.len() {
        let (index, file_name, result) = load_rx
            .recv()
            .map_err(|_| anyhow::anyhow!("Loading workers disconnected"))?;

        let (file_info, frame_map) = result?;

        let mut file_progress = progress.add_child(format!("Compare {file_name}"));
        file_progress.init(None, None);

        let map = Arc::clone(&reference_map);
        let compare_thread =
            thread::spawn(move || ddm_app::compare(&map, &frame_map, &mut file_progress));

        compare_threads.push((index, file_name, file_info, Some(compare_thread)));
    }

    for load_thread in load_threads {
        load_thread
            .join()
            .map_err(|_| anyhow::anyhow!("Load completion thread panicked"))?;
    }

    let mut comparison_info = compare_threads
        .into_iter()
        .map(|(index, file_name, file_info, compare_thread)| {
            let result = match compare_thread {
                Some(handle) => handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("Compare thread panicked"))?,

                None => None,
            };

            Ok((index, file_name, file_info, result))
        })
        .collect::<Result<Vec<_>, anyhow::Error>>()?;

    // Results arrive in completion order, but display them in input order.
    comparison_info.sort_by_key(|(index, ..)| *index);

    let comparison_info = comparison_info
        .into_iter()
        .map(|(_, file_name, file_info, result)| (file_name, file_info, result))
        .collect::<Vec<_>>();

    Ok(format_comparison_info(&comparison_info))
}
