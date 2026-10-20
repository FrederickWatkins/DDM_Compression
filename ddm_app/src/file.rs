//! File handling functions
use std::{
    fs::File,
    io::{BufRead as _, BufReader, BufWriter},
    path::PathBuf,
};

use binrw::{BinRead, BinWrite};
use ddm_core::error::Error;

/// Load series of T from file
///
/// # Errors
/// Errors if the file isn't able to be loaded, or if it contains invalid data
pub fn load<T: BinRead + binrw::meta::ReadEndian>(
    path: PathBuf,
) -> Result<impl Iterator<Item = Result<T, Error>>, Error>
where
    for<'a> <T as BinRead>::Args<'a>: Default,
{
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    Ok(std::iter::from_fn(move || {
        match reader.fill_buf() {
            Ok([]) => return None,
            Ok(_) => {}
            Err(err) => return Some(Err(Error::from(err))),
        }

        Some(T::read(&mut reader).map_err(Error::from))
    }))
}

/// Save series of T to file
///
/// # Errors
/// Errors if the file can't be created
pub fn save<T: BinWrite + binrw::meta::WriteEndian, I: Iterator<Item = T>>(
    path: PathBuf,
    data: I,
) -> Result<(), Error>
where
    for<'a> <T as BinWrite>::Args<'a>: Default,
{
    let mut file = File::create(path)?;
    let mut writer = BufWriter::new(&mut file);
    for packet in data {
        packet.write(&mut writer)?;
    }

    Ok(())
}
