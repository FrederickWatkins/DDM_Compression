use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

use crate::l0b_packet::mp00::Mp00;

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct DdmFrame {
    pub channel_num: u8,
    pub timestamp_seconds: u32,
    pub timestamp_clocks: i32,
    #[serde(with = "BigArray")]
    pub pixel_data: [[u16; 20]; 52],
}

impl From<Mp00> for DdmFrame {
    fn from(value: Mp00) -> Self {
        let mut pixel_data = [[0u16; 20]; 52];

        // Copy each 20-element chunk into the corresponding row
        for (i, chunk) in value.pixel_data.chunks(20).enumerate() {
            pixel_data[i].copy_from_slice(chunk);
        }

        Self {
            channel_num: value.channel_num,
            timestamp_seconds: value.timestamp_seconds,
            timestamp_clocks: value.timestamp_clocks,
            pixel_data,
        }
    }
}
