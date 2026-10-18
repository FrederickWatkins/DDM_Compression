//! MT00 calibration packet

use binrw::{BinRead, BinWrite};

/// MT00 packet type
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[bw(big)]
pub struct MT00Packet {
    buf: [u8; 56]
}
