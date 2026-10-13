//! MT00 packet type
use super::Decode;

use anyhow::Result;

/// MT00 packet type
///
/// I have no idea what this does, something calibration related apparently
///
/// All I know is that when encoded it's 64 bytes long including the preamble
#[derive(Debug, Clone, PartialEq)]
pub struct MT00;

impl Decode for MT00 {
    fn decode<R: std::io::prelude::Read + std::io::prelude::Seek>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 56];
        reader.read_exact(&mut buf)?;
        Ok(MT00)
    }
}
