//! Experimental MQ00 packet type
use crate::DDMPixelData;

use super::{Decode, Encode};
pub mod cabac;

/// Experimental MQ00 packet type
pub struct MQ00 {
    channel_num: u8,
    status_flag: u8,
    pixel_quantisation: u8,
    timestamp_seconds: u32,
    timestamp_clocks: i32,
    pixel_data: DDMPixelData,
}

impl Encode for MQ00 {
    fn encode<W: std::io::prelude::Write>(&self, writer: &mut W) -> anyhow::Result<()> {
        todo!()
    }
}

impl Decode for MQ00 {
    fn decode<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
    ) -> anyhow::Result<Self> {
        todo!()
    }
}
