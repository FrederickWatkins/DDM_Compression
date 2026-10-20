//! File handling functions
use std::{
    fs::File,
    io::{BufRead as _, BufReader, BufWriter, Seek as _},
    path::PathBuf,
};

use binrw::{BinRead, BinWrite};
use ddm_core::error::Error;
use itertools::Itertools as _;
use prodash::unit::{self, Bytes, Human, display::Mode};

/// Load series of T from file
///
/// # Errors
/// Errors if the file isn't able to be loaded, or if it contains invalid data
pub fn load<T: BinRead + binrw::meta::ReadEndian>(
    path: PathBuf,
    progress: &mut dyn prodash::Progress,
) -> Result<impl Iterator<Item = Result<T, Error>>, Error>
where
    for<'a> <T as BinRead>::Args<'a>: Default,
{
    let file = File::open(path)?;
    let len = file.metadata()?.len();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Loading bar truncation doesn't matter"
    )]
    progress.init(
        Some(len.min(usize::MAX as u64) as usize),
        Some(unit::dynamic_and_mode(
            Bytes,
            Mode::with_throughput().and_percentage(),
        )),
    );
    let mut reader = BufReader::new(file);

    Ok(std::iter::from_fn(move || {
        match reader.fill_buf() {
            Ok([]) => return None,
            Ok(_) => {}
            Err(err) => return Some(Err(Error::from(err))),
        }

        let result = T::read(&mut reader).map_err(Error::from);
        if let Ok(position) = reader.stream_position() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "Loading bar truncation doesn't matter"
            )]
            progress.set(position.min(usize::MAX as u64) as usize);
        }
        Some(result)
    }))
}

/// Save series of T to file
///
/// # Errors
/// Errors if the file can't be created
pub fn save<T: BinWrite + binrw::meta::WriteEndian, I: Iterator<Item = T>>(
    path: PathBuf,
    data: I,
    progress: &mut dyn prodash::Progress,
) -> Result<(), Error>
where
    for<'a> <T as BinWrite>::Args<'a>: Default,
{
    let mut formatter = human_format::Formatter::new();
    formatter.with_decimals(2);
    progress.init(
        data.try_len().ok(),
        Some(unit::dynamic_and_mode(
            Human::new(formatter, "packets"),
            Mode::with_throughput().and_percentage(),
        )),
    );
    let mut file = File::create(path)?;
    let mut writer = BufWriter::new(&mut file);
    for packet in data {
        packet.write(&mut writer)?;
        progress.inc();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use ddm_core::l0b::L0bPacket;
    use prodash::progress::Discard;
    use proptest::prelude::*;
    use tempfile::tempdir;

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 16, .. ProptestConfig::default() })]

        #[test]
        fn packet_stream_round_trip(packets in proptest::collection::vec(any::<L0bPacket<20, 52>>(), 1..10)) {
            let temp_dir = tempdir()?;
            let path = temp_dir.path().join("packets.l0b.ddm");
            let mut progress = Discard;

            save(path.clone(), packets.iter().cloned(), &mut progress)?;
            let loaded_packets = load::<L0bPacket<20, 52>>(path, &mut progress)?
                .collect::<Result<Vec<_>, _>>()?;

            prop_assert_eq!(packets, loaded_packets);
        }
    }
}
