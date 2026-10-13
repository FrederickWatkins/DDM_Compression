use crate::{DDM_HEIGHT, DDM_WIDTH, DDMPixelData};

use super::Decode;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};

const NUM_PIXELS: usize = DDM_WIDTH * DDM_HEIGHT;

#[derive(Debug, Clone, PartialEq)]
pub struct Mp00 {
    pub header_version: u8,
    pub channel_num: u8,
    pub status_flag: u8,
    pub pixel_quantisation: u8,
    pub timestamp_seconds: u32,
    pub timestamp_clocks: i32,
    pub pixel_data: DDMPixelData,
    pub crc: u32,
}

impl Decode for Mp00 {
    /// Deserializes a byte buffer or stream in the VRQ binary format into `mp00`.
    fn decode<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let mut header_buf = [0u8; 12];

        // Reading the header is what determines whether another packet exists.
        // If there isn't a complete header, we're at EOF.
        reader.read_exact(&mut header_buf)?;
        let header_version = header_buf[0];
        let channel_num = header_buf[1];
        let status_flag = header_buf[2];
        let pixel_quantisation = header_buf[3];
        let timestamp_seconds = u32::from_be_bytes(header_buf[4..8].try_into().unwrap());
        let timestamp_clocks = i32::from_be_bytes(header_buf[8..12].try_into().unwrap());
        todo!()
    }
}

/// Sign extends an N-bit integer value stored in `value` to a full 32-bit signed integer.
fn sign_extend(value: i32, bits: u32) -> i32 {
    let shift = 32 - bits;
    (value << shift) >> shift
}
