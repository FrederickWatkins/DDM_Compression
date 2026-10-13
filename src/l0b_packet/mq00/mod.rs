use crate::DDMPixelData;

use super::{Encode, Decode};
pub mod cabac;

pub struct Mq00 {
    pub header_version: u8,
    pub channel_num: u8,
    pub status_flag: u8,
    pub pixel_quantisation: u8,
    pub timestamp_seconds: u32,
    pub timestamp_clocks: i32,
    pub pixel_data: DDMPixelData,
    pub crc: u32,
}

impl Encode for Mq00 {
    fn encode<W: std::io::prelude::Write>(self, writer: &mut W) -> anyhow::Result<()> {
        todo!()
    }
}

impl Decode for Mq00 {
    fn decode<R: std::io::prelude::Read + std::io::prelude::Seek>(reader: &mut R) -> anyhow::Result<Self> {
        todo!()
    }
}