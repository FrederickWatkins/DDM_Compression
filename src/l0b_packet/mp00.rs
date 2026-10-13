//! MP00 packet type
//!
//! Existing SSTL solution using delta encoding in pixel encode order
//! ```
//! use ddm_compression::{
//!     DDMPixelData,
//!     l0b_packet::{Decode, Encode, mp00::MP00},
//! };
//! use rand::rngs::StdRng;
//! use rand::{RngExt, SeedableRng};
//! use std::io::Cursor;
//!
//! let mut pixel_data: DDMPixelData = [[0; _]; _];
//!
//! let mut rng = StdRng::seed_from_u64(42);
//! for row in &mut pixel_data {
//!     for pixel in row {
//!         *pixel = rng.random_range(1000..1200);
//!     }
//! }
//!
//! let original_packet = MP00::new(2, 0, 1, 452, 4356, pixel_data);
//! let original_size = size_of::<MP00>();
//! let mut buffer = vec![];
//! original_packet.encode(&mut buffer).unwrap();
//!
//! let encoded_size = buffer.len();
//!
//! let mut cursor = Cursor::new(buffer);
//! let decoded_packet = MP00::decode(&mut cursor).unwrap();
//!
//! assert_eq!(original_packet, decoded_packet);
//!
//! println!("Original size: {original_size}");
//! println!("Encoded size: {encoded_size}");
//! println!(
//!     "Compression ratio: {:.2}",
//!     original_size as f32 / encoded_size as f32
//! );
//! // Output:
//! // Original size: 2092
//! // Encoded size: 1532
//! // Compression ratio: 1.37
//! ```

use crate::{
    DDMPixelData,
    l0b_packet::{Decode, Encode},
};

use anyhow::{Result, anyhow};
use std::io::{Read, Seek, Write};

const HEADER_VERSION: u8 = 2;

/// MP00 packet type
///
/// Existing solution using pixel delta in encoding order
#[derive(Debug, Clone, PartialEq)]
pub struct MP00 {
    channel_num: u8,
    status_flag: u8,
    pixel_quantisation: u8,
    timestamp_seconds: u32,
    timestamp_clocks: i32,
    pixel_data: DDMPixelData,
}

impl Decode for MP00 {
    fn decode<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let mut header_buf = [0u8; 12];

        reader.read_exact(&mut header_buf)?;
        let header_version = header_buf[0];
        let channel_num = header_buf[1];
        let status_flag = header_buf[2];
        let pixel_quantisation = header_buf[3];
        let timestamp_seconds = u32::from_be_bytes(header_buf[4..8].try_into().unwrap());
        let timestamp_clocks = i32::from_be_bytes(header_buf[8..12].try_into().unwrap());

        if header_version != HEADER_VERSION {
            return Err(anyhow!(
                "Illegal MP00 header version {header_version} found at byte {:x}\nOnly MP00 header version 2 is supported",
                reader.stream_position()?
            ));
        }

        let (pixel_data, bytes_decoded) = decode_pixel_data(reader)?;

        if !bytes_decoded.is_multiple_of(4) {
            let target_distance = 4 - (bytes_decoded % 4);
            reader.seek_relative(target_distance.try_into().unwrap())?;
        }
        let mut crc = [0u8; 4];
        reader.read_exact(&mut crc)?;
        let crc = u32::from_be_bytes(crc); // TODO check CRC
        Ok(Self {
            channel_num,
            status_flag,
            pixel_quantisation,
            timestamp_seconds,
            timestamp_clocks,
            pixel_data,
        })
    }
}

impl Encode for MP00 {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        let mut header_buf = [0u8; 12];

        header_buf[0] = HEADER_VERSION;
        header_buf[1] = self.channel_num;
        header_buf[2] = self.status_flag;
        header_buf[3] = self.pixel_quantisation;
        header_buf[4..8].copy_from_slice(&self.timestamp_seconds.to_be_bytes());
        header_buf[8..12].copy_from_slice(&self.timestamp_clocks.to_be_bytes());

        writer.write_all(&header_buf)?;

        let bytes_encoded = encode_pixel_data(writer, &self.pixel_data)?;

        if !bytes_encoded.is_multiple_of(4) {
            let target_distance = 4 - (bytes_encoded % 4);
            writer.write_all(&vec![0u8; target_distance])?;
        }
        let crc = 0u32; // TODO write actual CRC code
        writer.write_all(&crc.to_be_bytes())?;
        Ok(())
    }
}

impl MP00 {
    /// Create new MP00 packet from raw attributes
    pub fn new(
        channel_num: u8,
        status_flag: u8,
        pixel_quantisation: u8,
        timestamp_seconds: u32,
        timestamp_clocks: i32,
        pixel_data: DDMPixelData,
    ) -> Self {
        Self {
            channel_num,
            status_flag,
            pixel_quantisation,
            timestamp_seconds,
            timestamp_clocks,
            pixel_data,
        }
    }

    /// DDM hardware channel number
    pub fn channel_num(&self) -> u8 {
        self.channel_num
    }

    /// Time stamp of integration start: GPS seconds at last PPS as rounded integer
    pub fn timestamp_seconds(&self) -> u32 {
        self.timestamp_seconds
    }

    /// Time stamp of integration start: sample clocks since last PPS
    pub fn timestamp_clocks(&self) -> i32 {
        self.timestamp_clocks
    }

    /// DDM pixel data
    pub fn pixel_data(&self) -> DDMPixelData {
        self.pixel_data
    }
}

/// Decode pixel data using sign extended VRQ format
///
/// Returns pixel data and number of bytes read
fn decode_pixel_data<R: Read>(reader: &mut R) -> Result<(DDMPixelData, usize)> {
    let mut output: DDMPixelData = [[0; _]; _];
    let mut bytes_read = 0;
    let mut prev_pixel = 0u16;
    for row in &mut output {
        for pixel in row {
            let (delta_value, extra_bytes) = decode_vrq(reader)?;
            *pixel = prev_pixel.overflowing_add(delta_value.cast_unsigned()).0;
            prev_pixel = *pixel;
            bytes_read += extra_bytes;
        }
    }
    Ok((output, bytes_read))
}

/// Encode pixel data using sign extended VRQ format
///
/// Returns number of bytes written
fn encode_pixel_data<W: Write>(writer: &mut W, pixel_data: &DDMPixelData) -> Result<usize> {
    let mut bytes_written = 0;
    let mut prev_pixel = 0u16;
    for row in pixel_data {
        for pixel in row {
            let delta_value: i16 = pixel.overflowing_sub(prev_pixel).0.cast_signed();
            let extra_bytes = encode_vrq(writer, delta_value)?;
            prev_pixel = *pixel;
            bytes_written += extra_bytes;
        }
    }
    Ok(bytes_written)
}

/// Decode an i16 from 3 byte VRQ format
///
/// Returns result and number of bytes read
fn decode_vrq<R: Read>(reader: &mut R) -> Result<(i16, usize)> {
    let mut bytes = [0u8; 3];
    reader.read_exact(&mut bytes[0..1])?;
    if bytes[0] >> 7 == 1 {
        reader.read_exact(&mut bytes[1..2])?;
        if bytes[1] >> 7 == 1 {
            reader.read_exact(&mut bytes[2..3])?;
            // Pass width 16 for 3-byte i16 values
            Ok((sign_extend(&bytes, 16), 3))
        } else {
            Ok((sign_extend(&bytes, 14), 2))
        }
    } else {
        Ok((sign_extend(&bytes, 7), 1))
    }
}

/// Encode an i16 into 3 byte VRQ format
///
/// Returns number of bytes written
fn encode_vrq<W: Write>(writer: &mut W, val: i16) -> Result<usize> {
    if (-64..=63).contains(&val) {
        let b0 = u8::try_from(val.cast_unsigned() & 0x7F).unwrap();
        writer.write_all(&[b0])?;
        Ok(1)
    } else if (-8192..=8191).contains(&val) {
        let u = val.cast_unsigned();
        let b0 = u8::try_from(u & 0x7F).unwrap() | 0x80;
        let b1 = u8::try_from((u >> 7) & 0x7F).unwrap();
        writer.write_all(&[b0, b1])?;
        Ok(2)
    } else {
        let u = u32::from(val.cast_unsigned());
        let b0 = u8::try_from(u & 0x7F).unwrap() | 0x80;
        let b1 = u8::try_from((u >> 7) & 0x7F).unwrap() | 0x80;
        let b2 = u8::try_from(u >> 14).unwrap() & 0x7F;
        writer.write_all(&[b0, b1, b2])?;
        Ok(3)
    }
}

/// Sign extend a signed integer of arbitrary width, using only the lower seven bits in each byte
fn sign_extend(bytes: &[u8], width: usize) -> i16 {
    let mut output = 0i16;
    let num_bytes = bytes.len();
    for (i, byte) in bytes.iter().enumerate().take(num_bytes) {
        output |= i16::from(byte & 0x7F) << (7 * i);
    }
    // Only sign-extend if width is less than 16
    if width < 16 && (output >> (width - 1)) & 1 == 1 {
        output |= -1 << width;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::arb_pixel_data;
    use proptest::prelude::*;
    use std::io::Cursor;

    // Custom strategy to generate arbitrary valid Mp00 instances
    pub fn arb_mp00(
        pixel_data_strategy: impl Strategy<Value = u16> + Clone,
    ) -> impl Strategy<Value = MP00> {
        (
            any::<u8>(),
            any::<u8>(),
            any::<u8>(),
            any::<u32>(),
            any::<i32>(),
            arb_pixel_data(pixel_data_strategy),
        )
            .prop_map(
                |(
                    channel_num,
                    status_flag,
                    pixel_quantisation,
                    timestamp_seconds,
                    timestamp_clocks,
                    pixel_data,
                )| {
                    MP00 {
                        channel_num,
                        status_flag,
                        pixel_quantisation,
                        timestamp_seconds,
                        timestamp_clocks,
                        pixel_data,
                    }
                },
            )
    }

    proptest! {
        #[test]
        fn mp00_round_trip(original in arb_mp00(any::<u16>())) {
            // Encode the packet into an in-memory buffer
            let mut buffer = vec![];
            original.encode(&mut buffer).unwrap();

            // Decode the packet back from the buffer
            let mut cursor = Cursor::new(buffer);
            let decoded = MP00::decode(&mut cursor).unwrap();

            // Ensure round-trip integrity holds
            prop_assert_eq!(original, decoded);
        }

        #[test]
        fn pixel_data_round_trip(original in arb_pixel_data(any::<u16>())) {
            // Encode the packet into an in-memory buffer
            let mut buffer = vec![];
            encode_pixel_data(&mut buffer, &original).unwrap();

            // Decode the packet back from the buffer
            let mut cursor = Cursor::new(buffer);
            let decoded = decode_pixel_data(&mut cursor).unwrap().0;

            // Ensure round-trip integrity holds
            prop_assert_eq!(original, decoded);
        }

        #[test]
        fn vrq_round_trip(original in any::<i16>()) {
            // Encode the packet into an in-memory buffer
            let mut buffer = vec![];
            encode_vrq(&mut buffer, original).unwrap();

            // Decode the packet back from the buffer
            let mut cursor = Cursor::new(buffer);
            let decoded = decode_vrq(&mut cursor).unwrap().0;

            // Ensure round-trip integrity holds
            prop_assert_eq!(original, decoded);
        }
    }
}
