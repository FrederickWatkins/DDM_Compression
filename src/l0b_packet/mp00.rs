use super::Decode;
use std::io::{self, Read, Seek, SeekFrom};

#[derive(Debug, Clone, PartialEq)]
pub struct Mp00 {
    pub header_version: u8,
    pub channel_num: u8,
    pub status_flag: u8,
    pub pixel_quatisation: u8,
    pub timestamp_seconds: u32,
    pub timestamp_clocks: i32,
    pub pixel_data: Vec<u16>,
}

impl Decode for Mp00 {
    /// Deserializes a byte buffer or stream in the VRQ binary format into `mp00`.
    fn decode<R: Read + Seek>(reader: &mut R) -> io::Result<Self> {
        let mut header_buf = [0u8; 12];

        // Reading the header is what determines whether another packet exists.
        // If there isn't a complete header, we're at EOF.
        reader.read_exact(&mut header_buf)?;
        let header_version = header_buf[0];
        let channel_num = header_buf[1];
        let status_flag = header_buf[2];
        let pixel_quatisation = header_buf[3];
        let timestamp_seconds = u32::from_be_bytes(header_buf[4..8].try_into().unwrap());
        let timestamp_clocks = i32::from_be_bytes(header_buf[8..12].try_into().unwrap());

        const NUM_PIXELS: usize = 52 * 20;

        let mut pixel_data = Vec::with_capacity(NUM_PIXELS);
        let mut last_pixel: i32 = 0;

        while pixel_data.len() < NUM_PIXELS {
            let mut buf = [0u8; 1];
            reader.read_exact(&mut buf)?;
            let read_byte = buf[0];

            let delta_decoded = if read_byte >= 128 {
                reader.read_exact(&mut buf)?;
                let read_byte2 = buf[0];

                if read_byte2 >= 128 {
                    reader.read_exact(&mut buf)?;
                    let read_byte3 = buf[0];

                    // 3-byte VRQ (22-bit signed word)
                    let reconstructed = ((read_byte3 as i32) << 14)
                        | (((read_byte2 & 0x7F) as i32) << 7)
                        | ((read_byte & 0x7F) as i32);

                    let signed = sign_extend(reconstructed, 22);
                    last_pixel + signed
                } else {
                    // 2-byte VRQ (14-bit signed word)
                    let reconstructed =
                        (((read_byte2 & 0x7F) as i32) << 7) | ((read_byte & 0x7F) as i32);

                    let signed = sign_extend(reconstructed, 14);
                    last_pixel + signed
                }
            } else {
                // 1-byte VRQ (7-bit signed word)
                let signed = sign_extend(read_byte as i32, 7);
                last_pixel + signed
            };

            last_pixel = delta_decoded;

            let final_pixel = (delta_decoded << pixel_quatisation) as u16;
            pixel_data.push(final_pixel);
        }

        let pos = reader.stream_position().unwrap();
        let padding = (4 - (pos % 4)) % 4;

        if padding != 0 {
            reader.seek(SeekFrom::Current(padding as i64)).unwrap();
        }

        // CRC comes immediately after the pixel payload.
        let mut crc_buf = [0u8; 4];
        reader.read_exact(&mut crc_buf)?;

        let _crc = u32::from_be_bytes(crc_buf);

        Ok(Mp00 {
            header_version,
            channel_num,
            status_flag,
            pixel_quatisation,
            timestamp_seconds,
            timestamp_clocks,
            pixel_data,
        })
    }
}

/// Sign extends an N-bit integer value stored in `value` to a full 32-bit signed integer.
fn sign_extend(value: i32, bits: u32) -> i32 {
    let shift = 32 - bits;
    (value << shift) >> shift
}
