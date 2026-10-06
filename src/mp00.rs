use std::io::{self, Read, Write, Seek, SeekFrom};

pub struct Mp00 {
    pub packet_preamble: u32,
    pub packet_type: u32,
    pub header_version: u8,
    pub channel_num: u8,
    pub status_flag: u8,
    pub pixel_quatisation: u8,
    pub timestamp_seconds: u32,
    pub timestamp_clocks: i32,
    pub pixel_data: Vec<u16>,
}

impl Mp00 {
    /// Deserializes a byte buffer or stream in the VRQ binary format into `mp00`.
    pub fn deserialize<R: Read + Seek>(reader: &mut R) -> io::Result<Self> {
        let mut header_buf = [0u8; 20];

        // Reading the header is what determines whether another packet exists.
        // If there isn't a complete header, we're at EOF.
        reader.read_exact(&mut header_buf)?;

        let packet_preamble = u32::from_be_bytes(header_buf[0..4].try_into().unwrap());
        let packet_type = u32::from_be_bytes(header_buf[4..8].try_into().unwrap());
        let header_version = header_buf[8];
        let channel_num = header_buf[9];
        let status_flag = header_buf[10];
        let pixel_quatisation = header_buf[11];
        let timestamp_seconds = u32::from_be_bytes(header_buf[12..16].try_into().unwrap());
        let timestamp_clocks = i32::from_be_bytes(header_buf[16..20].try_into().unwrap());

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
                        (((read_byte2 & 0x7F) as i32) << 7)
                        | ((read_byte & 0x7F) as i32);

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
            println!("Skipping {} padding bytes", padding);
            reader.seek(SeekFrom::Current(padding as i64)).unwrap();
        }

        // CRC comes immediately after the pixel payload.
        let mut crc_buf = [0u8; 4];
        reader.read_exact(&mut crc_buf)?;

        let _crc = u32::from_be_bytes(crc_buf);

        Ok(Mp00 {
            packet_preamble,
            packet_type,
            header_version,
            channel_num,
            status_flag,
            pixel_quatisation,
            timestamp_seconds,
            timestamp_clocks,
            pixel_data,
        })
    }

    /// Serializes the `mp00` struct into the VRQ binary format.
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let mut packet_body = Vec::new();

        // 1. Write Header (20 Bytes)
        packet_body.extend_from_slice(&self.packet_preamble.to_be_bytes());
        packet_body.extend_from_slice(&self.packet_type.to_be_bytes());
        packet_body.push(self.header_version);
        packet_body.push(self.channel_num);
        packet_body.push(self.status_flag);
        packet_body.push(self.pixel_quatisation);
        packet_body.extend_from_slice(&self.timestamp_seconds.to_be_bytes());
        packet_body.extend_from_slice(&self.timestamp_clocks.to_be_bytes());

        // 2. Encode DDM Pixels (VRQ + Delta Encoding)
        let mut last_pixel: i32 = 0;
        for &pixel in &self.pixel_data {
            let pixel_val = (pixel >> self.pixel_quatisation) as i32;
            let delta = pixel_val - last_pixel;
            last_pixel = pixel_val;

            if delta >= -64 && delta <= 63 {
                // 1-byte encoding
                let b0 = (delta & 0x7F) as u8;
                packet_body.push(b0);
            } else if delta >= -8192 && delta <= 8191 {
                // 2-byte encoding
                let b0 = (delta & 0x7F) as u8 | 0x80;
                let b1 = ((delta >> 7) & 0x7F) as u8;
                packet_body.push(b0);
                packet_body.push(b1);
            } else {
                // 3-byte encoding (supports -2,097,152 to 2,097,151)
                let b0 = (delta & 0x7F) as u8 | 0x80;
                let b1 = ((delta >> 7) & 0x7F) as u8 | 0x80;
                let b2 = ((delta >> 14) & 0xFF) as u8;
                packet_body.push(b0);
                packet_body.push(b1);
                packet_body.push(b2);
            }
        }

        // 3. Compute and append CRC checksum
        let crc = compute_crc32(&packet_body);
        packet_body.extend_from_slice(&crc.to_be_bytes());

        writer.write_all(&packet_body)?;
        Ok(())
    }
}

/// Sign extends an N-bit integer value stored in `value` to a full 32-bit signed integer.
fn sign_extend(value: i32, bits: u32) -> i32 {
    let shift = 32 - bits;
    (value << shift) >> shift
}

/// Dummy/Placeholder CRC checksum function (replace with specific target CRC algorithm if required)
fn compute_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}