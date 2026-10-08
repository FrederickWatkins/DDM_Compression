mod ddm_frame;
mod l0b_packet;

use std::fs::File;
use std::io::{BufReader, Cursor, Seek};

use crate::ddm_frame::DdmFrame;
use crate::l0b_packet::{Decode, L0bPacket, L0bPacketType};

use arcode::{
    ArithmeticDecoder, ArithmeticEncoder, EOFKind, Model,
    bitbit::{BitReader, BitWriter, MSB},
};

use osclet::{DwtExecutor, DwtInverseExecutor, Osclet};

const NZ: usize = 32;
const NY: usize = 52;
const NX: usize = 20;

type Data3D = [[[i32; NX]; NY]; NZ];

fn arithmetic_encode_bytes(data: &[u8]) -> Vec<u8> {
    let mut model = Model::builder()
        .num_bits(8)
        .eof(EOFKind::EndAddOne)
        .build();

    let compressed = Cursor::new(Vec::<u8>::new());
    let mut writer = BitWriter::<_>::new(compressed);

    let mut encoder = ArithmeticEncoder::new(48);

    for &byte in data {
        encoder
            .encode(byte as u32, &model, &mut writer)
            .expect("arithmetic encoding failed");

        model.update_symbol(byte as u32);
    }

    encoder
        .encode(model.eof(), &model, &mut writer)
        .expect("arithmetic EOF encoding failed");

    encoder
        .finish_encode(&mut writer)
        .expect("arithmetic encoder finish failed");

    writer
        .pad_to_byte()
        .expect("arithmetic encoder padding failed");

    writer.get_ref().get_ref().clone()
}

fn arithmetic_decode_bytes(data: &[u8]) -> Vec<u8> {
    let mut model = Model::builder()
        .num_bits(8)
        .eof(EOFKind::EndAddOne)
        .build();

    let mut reader = BitReader::<_, MSB>::new(data);
    let mut decoder = ArithmeticDecoder::new(48);
    let mut decoded = Vec::new();

    while !decoder.finished() {
        let symbol = decoder
            .decode(&model, &mut reader)
            .expect("arithmetic decoding failed");

        model.update_symbol(symbol);

        if symbol != model.eof() {
            decoded.push(symbol as u8);
        }
    }

    decoded
}

/// Arithmetic-code the raw u16 samples.
///
/// Unlike the wavelet path, the samples are encoded directly as 16-bit
/// arithmetic symbols -- there is no prediction, transform, delta coding,
/// etc.
fn arithmetic_encode_u16(data: &[u16]) -> Vec<u8> {
    let mut model = Model::builder()
        .num_bits(16)
        .eof(EOFKind::EndAddOne)
        .build();

    let compressed = Cursor::new(Vec::<u8>::new());
    let mut writer = BitWriter::<_>::new(compressed);

    let mut encoder = ArithmeticEncoder::new(48);

    for &sample in data {
        encoder
            .encode(sample as u32, &model, &mut writer)
            .expect("arithmetic encoding failed");

        model.update_symbol(sample as u32);
    }

    encoder
        .encode(model.eof(), &model, &mut writer)
        .expect("arithmetic EOF encoding failed");

    encoder
        .finish_encode(&mut writer)
        .expect("arithmetic encoder finish failed");

    writer
        .pad_to_byte()
        .expect("arithmetic encoder padding failed");

    writer.get_ref().get_ref().clone()
}

fn arithmetic_decode_u16(data: &[u8]) -> Vec<u16> {
    let mut model = Model::builder()
        .num_bits(16)
        .eof(EOFKind::EndAddOne)
        .build();

    let mut reader = BitReader::<_, MSB>::new(data);
    let mut decoder = ArithmeticDecoder::new(48);
    let mut decoded = Vec::new();

    while !decoder.finished() {
        let symbol = decoder
            .decode(&model, &mut reader)
            .expect("arithmetic decoding failed");

        model.update_symbol(symbol);

        if symbol != model.eof() {
            decoded.push(symbol as u16);
        }
    }

    decoded
}

fn dwt_x(
    input: &Data3D,
    output: &mut Data3D,
    dwt: &dyn DwtExecutor<i32>,
) {
    for z in 0..NZ {
        for y in 0..NY {
            let mut line = [0i32; NX];

            for x in 0..NX {
                line[x] = input[z][y][x];
            }

            let result = dwt
                .dwt(&line, 1)
                .expect("CDF 5/3 DWT failed along X");

            let split = result.approximations.len();

            for x in 0..split {
                output[z][y][x] = result.approximations[x];
            }

            for x in 0..result.details.len() {
                output[z][y][split + x] = result.details[x];
            }
        }
    }
}

/// Apply one 1-D CDF 5/3 transform to every line along Y.
fn dwt_y(
    input: &Data3D,
    output: &mut Data3D,
    dwt: &dyn DwtExecutor<i32>,
) {
    for z in 0..NZ {
        for x in 0..NX {
            let mut line = [0i32; NY];

            for y in 0..NY {
                line[y] = input[z][y][x];
            }

            let result = dwt
                .dwt(&line, 1)
                .expect("CDF 5/3 DWT failed along Y");

            let split = result.approximations.len();

            for y in 0..split {
                output[z][y][x] = result.approximations[y];
            }

            for y in 0..result.details.len() {
                output[z][split + y][x] = result.details[y];
            }
        }
    }
}

/// Apply one 1-D CDF 5/3 transform to every line along Z.
fn dwt_z(
    input: &Data3D,
    output: &mut Data3D,
    dwt: &dyn DwtExecutor<i32>,
) {
    for y in 0..NY {
        for x in 0..NX {
            let mut line = [0i32; NZ];

            for z in 0..NZ {
                line[z] = input[z][y][x];
            }

            let result = dwt
                .dwt(&line, 1)
                .expect("CDF 5/3 DWT failed along Z");

            let split = result.approximations.len();

            for z in 0..split {
                output[z][y][x] = result.approximations[z];
            }

            for z in 0..result.details.len() {
                output[split + z][y][x] = result.details[z];
            }
        }
    }
}

/// Inverse of the Z pass.
fn idwt_z(
    input: &Data3D,
    output: &mut Data3D,
    dwt: &dyn DwtExecutor<i32>,
) {
    let split = (NZ + 1) / 2;

    for y in 0..NY {
        for x in 0..NX {
            let mut approx = [0i32; (NZ + 1) / 2];
            let mut details = [0i32; NZ / 2];
            let mut line = [0i32; NZ];

            for z in 0..split {
                approx[z] = input[z][y][x];
            }

            for z in 0..(NZ / 2) {
                details[z] = input[split + z][y][x];
            }

            dwt.execute_inverse(&approx, &details, &mut line)
                .expect("CDF 5/3 inverse DWT failed along Z");

            for z in 0..NZ {
                output[z][y][x] = line[z];
            }
        }
    }
}

/// Inverse of the Y pass.
fn idwt_y(
    input: &Data3D,
    output: &mut Data3D,
    dwt: &dyn DwtExecutor<i32>,
) {
    let split = (NY + 1) / 2;

    for z in 0..NZ {
        for x in 0..NX {
            let mut approx = [0i32; (NY + 1) / 2];
            let mut details = [0i32; NY / 2];
            let mut line = [0i32; NY];

            for y in 0..split {
                approx[y] = input[z][y][x];
            }

            for y in 0..(NY / 2) {
                details[y] = input[z][split + y][x];
            }

            dwt.execute_inverse(&approx, &details, &mut line)
                .expect("CDF 5/3 inverse DWT failed along Y");

            for y in 0..NY {
                output[z][y][x] = line[y];
            }
        }
    }
}

/// Inverse of the X pass.
fn idwt_x(
    input: &Data3D,
    output: &mut Data3D,
    dwt: &dyn DwtExecutor<i32>,
) {
    let split = (NX + 1) / 2;

    for z in 0..NZ {
        for y in 0..NY {
            let mut approx = [0i32; (NX + 1) / 2];
            let mut details = [0i32; NX / 2];
            let mut line = [0i32; NX];

            for x in 0..split {
                approx[x] = input[z][y][x];
            }

            for x in 0..(NX / 2) {
                details[x] = input[z][y][split + x];
            }

            dwt.execute_inverse(&approx, &details, &mut line)
                .expect("CDF 5/3 inverse DWT failed along X");

            for x in 0..NX {
                output[z][y][x] = line[x];
            }
        }
    }
}

/// Perform a separable one-level 3-D CDF 5/3 transform.
///
/// The transform is:
///
///     X transform -> Y transform -> Z transform
///
/// Since each individual CDF 5/3 transform is reversible, the complete
/// 3-D transform is also reversible.
fn dwt_3d(input: &Data3D) -> Data3D {
    let dwt = Osclet::make_cdf53_i32();

    let mut x = [[[0i32; NX]; NY]; NZ];
    let mut y = [[[0i32; NX]; NY]; NZ];
    let mut z = [[[0i32; NX]; NY]; NZ];

    dwt_x(input, &mut x, dwt.as_ref());
    dwt_y(&x, &mut y, dwt.as_ref());
    dwt_z(&y, &mut z, dwt.as_ref());

    z
}

/// Perform the inverse of the separable one-level 3-D CDF 5/3 transform.
///
/// Forward order:
///     X -> Y -> Z
///
/// Therefore inverse order:
///     Z -> Y -> X
fn idwt_3d(coefficients: &Data3D) -> Data3D {
    let dwt = Osclet::make_cdf53_i32();

    let mut y = [[[0i32; NX]; NY]; NZ];
    let mut x = [[[0i32; NX]; NY]; NZ];
    let mut output = [[[0i32; NX]; NY]; NZ];

    idwt_z(coefficients, &mut y, dwt.as_ref());
    idwt_y(&y, &mut x, dwt.as_ref());
    idwt_x(&x, &mut output, dwt.as_ref());

    output
}

/// Map signed integers to unsigned integers:
///
///   0  -> 0
///  -1 -> 1
///   1 -> 2
///  -2 -> 3
///   2 -> 4
///   ...
///
/// This makes values close to zero close to zero after mapping.
#[inline]
fn zigzag_i32(x: i32) -> u32 {
    ((x << 1) ^ (x >> 31)) as u32
}

/// Encode an unsigned integer as ULEB128.
///
/// Small coefficients therefore consume fewer bytes:
///
///   0..127          -> 1 byte
///   128..16383      -> 2 bytes
///   ...
#[inline]
fn write_uleb128(mut value: u32, output: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;

        if value != 0 {
            byte |= 0x80;
        }

        output.push(byte);

        if value == 0 {
            break;
        }
    }
}

fn coefficients_to_bytes(coefficients: &Data3D) -> Vec<u8> {
    let mut bytes = Vec::new();

    // Reserve considerably less than 4 bytes/coefficient because
    // wavelet coefficients should usually be concentrated near zero.
    bytes.reserve(NZ * NY * NX);

    for z in 0..NZ {
        for y in 0..NY {
            for x in 0..NX {
                let value = zigzag_i32(coefficients[z][y][x]);
                write_uleb128(value, &mut bytes);
            }
        }
    }

    bytes
}

fn main() {
    let path = "data/HG1_003014_20260829_102905.l0b.ddm";

    let file = File::open(path).expect("Failed to open VRQ test file");
    let mut reader = BufReader::new(file);

    let mut packet_number = 0;

    let mut ddm_frames: Vec<Vec<DdmFrame>> = vec![vec![]; 16];

    loop {
        let stream_position = reader.stream_position().unwrap();
        match L0bPacket::decode(&mut reader) {
            Ok(L0bPacket {
                packet_type: L0bPacketType::Mp00(mp00),
                ..
            }) => {
                packet_number += 1;
                ddm_frames[mp00.channel_num as usize].push(mp00.into());
            }

            Ok(L0bPacket {
                packet_type: L0bPacketType::Mt00(_),
                ..
            }) => (),

            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                eprintln!("Reached end of file.");
                break;
            }

            Err(e) => {
                eprintln!(
                    "Error reading packet {} at offset {stream_position}: {}",
                    packet_number + 1,
                    e
                );
                break;
            }
        }
    }

    println!("Total packets: {}", packet_number);

    let mut sum = 0usize;

    for (i, channel) in ddm_frames.iter().enumerate() {
        println!("Channel {i}: {} frames", channel.len());
        sum += channel.len();
    }
    println!("{sum}");

    let test_channel: [DdmFrame; NZ] = ddm_frames[0][0..NZ].try_into().unwrap();

    let test_data: [[[u16; 20]; 52]; NZ] = std::array::from_fn(|i| test_channel[i].pixel_data);

    let original: Data3D = std::array::from_fn(|z| {
        std::array::from_fn(|y| {
            std::array::from_fn(|x| test_data[z][y][x] as i32)
        })
    });

    let number_of_samples = NZ * NY * NX;

    // The original data consists of 16-bit unsigned samples.
    let original_bits = number_of_samples * u16::BITS as usize;

    let raw_samples: Vec<u16> = test_data
        .iter()
        .flat_map(|plane| plane.iter())
        .flat_map(|row| row.iter())
        .copied()
        .collect();

    let raw_encoded = arithmetic_encode_u16(&raw_samples);

        // Verify raw u16 arithmetic coding is lossless.
    let raw_decoded = arithmetic_decode_u16(&raw_encoded);

    assert_eq!(
        raw_decoded, raw_samples,
        "raw u16 arithmetic coding round-trip failed"
    );

    println!("Raw arithmetic round-trip: PASS");

    let wavelet = dwt_3d(&original);

    // Verify that the 3-D CDF 5/3 transform is exactly invertible.
    let reconstructed = idwt_3d(&wavelet);

    assert_eq!(
        reconstructed, original,
        "3-D CDF 5/3 DWT round-trip failed"
    );

    println!("DWT round-trip: PASS");

    let wavelet_bytes = coefficients_to_bytes(&wavelet);

    let wavelet_encoded = arithmetic_encode_bytes(&wavelet_bytes);

    // Verify arithmetic coding of the wavelet byte stream is lossless.
    let wavelet_decoded = arithmetic_decode_bytes(&wavelet_encoded);

    assert_eq!(
        wavelet_decoded, wavelet_bytes,
        "wavelet arithmetic coding round-trip failed"
    );

    println!("Wavelet arithmetic round-trip: PASS");

    let raw_bits = raw_encoded.len() * 8;
    let wavelet_bits = wavelet_encoded.len() * 8;

    let raw_ratio = original_bits as f64 / raw_bits as f64;
    let wavelet_ratio = original_bits as f64 / wavelet_bits as f64;

    let raw_percent = raw_bits as f64 / original_bits as f64 * 100.0;
    let wavelet_percent = wavelet_bits as f64 / original_bits as f64 * 100.0;

    let mut og_size_bytes = 0;
    for frame in test_data {
        for line in frame {
            let mut prev_pixel = 0;
            for pixel in line {
                let delta_coded = pixel as i32 - prev_pixel as i32;
                // println!("{delta_coded}");
                prev_pixel = pixel;
                if -64 <= delta_coded && delta_coded < 64 {
                    og_size_bytes += 1;
                } else if -8192 <= delta_coded && delta_coded < 8192 {
                    og_size_bytes += 2;
                } else {
                    og_size_bytes += 3;
                }
            }
        }
    }

    println!();
    println!("============================================================");
    println!("              ARITHMETIC CODING RESULTS");
    println!("============================================================");
    println!("Volume: {} x {} x {}", NZ, NY, NX);
    println!("Samples: {}", number_of_samples);
    println!("Original size: {} bits ({:.2} KiB)",
        original_bits,
        original_bits as f64 / 8.0 / 1024.0
    );
    println!();

    println!("Delta + VRQ {og_size_bytes}");
    println!();

    println!("Raw samples + arithmetic coding:");
    println!("  Encoded: {} bytes ({} bits)", raw_encoded.len(), raw_bits);
    println!("  Size:    {:.2}% of original", raw_percent);
    println!("  Ratio:   {:.3}:1", raw_ratio);
    println!();

    println!("3-D CDF 5/3 + arithmetic coding:");
    println!("  Coefficients: {} bytes before arithmetic coding",
        wavelet_bytes.len()
    );
    println!("  Encoded:      {} bytes ({} bits)",
        wavelet_encoded.len(),
        wavelet_bits
    );
    println!("  Size:         {:.2}% of original", wavelet_percent);
    println!("  Ratio:        {:.3}:1", wavelet_ratio);
    println!();

    println!("Improvement from 3-D DWT:");
    println!("  {:.2}% smaller than raw arithmetic-coded data",
        (1.0 - wavelet_bits as f64 / raw_bits as f64) * 100.0
    );

    println!("============================================================");
}
