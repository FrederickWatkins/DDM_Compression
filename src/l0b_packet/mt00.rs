//! MT00 packet type
use std::io::{Read, Seek, Write};

use crate::l0b_packet::{Decode, Encode};

use anyhow::Result;

/// MT00 packet type
///
/// I have no idea what this does, something calibration related apparently
///
/// All I know is that when encoded it's 64 bytes long including the preamble
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MT00 {
    buf: [u8; 56],
}

impl Decode for MT00 {
    fn decode<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 56];
        reader.read_exact(&mut buf)?;
        Ok(MT00 { buf })
    }
}

impl Encode for MT00 {
    fn encode<W: Write + Seek>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.buf)?;
        Ok(())
    }
}

impl MT00 {
    /// Create new MT00 packet from raw buffer
    pub fn new(buf: &[u8; 56]) -> Self {
        Self { buf: *buf }
    }

    /// Get raw buffer from packet
    pub fn buf(&self) -> [u8; 56] {
        self.buf
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use proptest::prelude::*;

    // Custom strategy to generate arbitrary valid MP00 instances
    pub fn arb_mt00() -> impl Strategy<Value = MT00> {
        any::<[u8; 56]>().prop_map(|buf| MT00::new(&buf))
    }
}
