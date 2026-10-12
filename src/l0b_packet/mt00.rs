use super::Decode;

use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct Mt00;

impl Decode for Mt00 {
    fn decode<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
    ) -> Result<Self> {
        let mut buf = [0u8; 56];
        reader.read_exact(&mut buf)?;
        Ok(Mt00)
    }
}