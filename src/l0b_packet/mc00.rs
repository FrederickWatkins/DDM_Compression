use super::{Encode, Decode};

pub struct PerFrameMetadata {
    pub status_flag: u8,
    pub pixel_quantisation: u8,
    pub timestamp_seconds: u32,
    pub timestamp_clocks: i32,
}

pub struct Mc00 {
    pub header_version: u8,
    pub channel_num: u8,
    pub num_frames: u8,
    pub pixel_data: Vec<u8>,
}

impl Encode for Mc00 {
    fn encode<W: std::io::prelude::Write>(self, writer: &mut W) -> anyhow::Result<()> {
        todo!()
    }
}

impl Decode for Mc00 {
    fn decode<R: std::io::prelude::Read + std::io::prelude::Seek>(reader: &mut R) -> anyhow::Result<Self> {
        todo!()
    }
}