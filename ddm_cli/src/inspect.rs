use std::{fmt::Display, fs::File, io::BufReader, path::Path, sync::mpsc, thread};

use ddm_core::l0b::L0bPacket;
use prodash::{
    NestedProgress, Progress,
    unit::{self, Bytes, Human, display::Mode},
};

use crate::{HEIGHT, WIDTH, table::format_file_info};

pub fn inspect<I: IntoIterator<Item = impl AsRef<Path>>, P>(
    input: I,
    progress: &mut P,
) -> anyhow::Result<impl Display>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    let mut threads = vec![];
    for file in input {
        let file_name = file
            .as_ref()
            .file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or(file.as_ref().to_string_lossy())
            .into_owned();
        let mut file_progress = progress.add_child(format!("Inspect {file_name}"));
        let mut read_bar = file_progress.add_child("Read packets");
        let mut inspect_bar = file_progress.add_child("Inspect metadata");
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
        inspect_bar.init(
            None,
            Some(unit::dynamic_and_mode(
                Human::new(formatter, "packets"),
                Mode::with_throughput().and_percentage(),
            )),
        );

        let mut packet_stream: (mpsc::SyncSender<L0bPacket<WIDTH, HEIGHT>>, _) =
            mpsc::sync_channel(100);
        let mut reader = BufReader::new(File::open(file.as_ref())?);
        let read_thread =
            thread::spawn(move || ddm_app::read(&mut reader, &mut packet_stream.0, &mut read_bar));
        let inspect_thread =
            thread::spawn(move || ddm_app::inspect(&mut packet_stream.1, &mut inspect_bar));
        threads.push((file_name, read_thread, inspect_thread, file_progress));
    }

    let mut file_info = vec![];

    for (file_name, read_thread, inspect_thread, file_progress) in threads {
        file_progress.done("Done".to_owned());
        read_thread
            .join()
            .map_err(|_| anyhow::anyhow!("Read thread panicked"))??;
        let output = inspect_thread
            .join()
            .map_err(|_| anyhow::anyhow!("Inspect thread panicked"))?;
        file_info.push((file_name, output));
    }
    Ok(format_file_info(&file_info))
}
