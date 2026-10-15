use std::{
    fs::File,
    io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ddm_compression::{
    DDMPixelData,
    l0b_packet::{Decode, Encode, L0bPacket, mp00, mt00},
};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

const NUM_PACKETS: usize = 15000;
const MT00_PROBABILITY: f64 = 0.1;

fn encode_multiple<W: Write + Seek>(writer: &mut W, packets: &[L0bPacket]) {
    for packet in packets {
        packet.encode(writer).unwrap();
    }
}

fn decode_multiple<R: Read + Seek>(reader: &mut R, count: usize) -> Vec<L0bPacket> {
    (0..count)
        .map(|_| L0bPacket::decode(reader).unwrap())
        .collect()
}

fn decode_all<R: Read + Seek>(reader: &mut R, limit: usize) -> (Vec<L0bPacket>, usize) {
    let end = reader.seek(SeekFrom::End(0)).unwrap();
    reader.seek(SeekFrom::Start(0)).unwrap();

    let mut packets = Vec::with_capacity(limit);

    while packets.len() < limit && reader.stream_position().unwrap() < end {
        packets.push(L0bPacket::decode(reader).unwrap());
    }

    (packets, reader.stream_position().unwrap() as usize)
}

fn make_packets(num_packets: usize) -> Vec<L0bPacket> {
    let mut rng = StdRng::seed_from_u64(42);

    (0..num_packets)
        .map(|_| {
            if rng.random_bool(MT00_PROBABILITY) {
                let mut buf = [0u8; 56];

                for byte in &mut buf {
                    *byte = rng.random();
                }

                L0bPacket::MT00(Box::new(mt00::MT00::new(&buf)))
            } else {
                let mut pixel_data: DDMPixelData = [[0; _]; _];

                for row in &mut pixel_data {
                    for pixel in row {
                        *pixel = rng.random_range(1000..1200);
                    }
                }

                L0bPacket::MP00(Box::new(mp00::MP00::new(
                    1, 0, 0, 6332524, 2354353, pixel_data,
                )))
            }
        })
        .collect()
}

pub fn l0b(c: &mut Criterion) {
    let packets = make_packets(NUM_PACKETS);

    let mut encoded = Vec::new();
    let mut cursor = Cursor::new(&mut encoded);
    encode_multiple(&mut cursor, &packets);

    let total_bytes = encoded.len();
    let mut group = c.benchmark_group("l0b");

    group.throughput(Throughput::ElementsAndBytes {
        elements: packets.len() as u64,
        bytes: total_bytes as u64,
    });
    group.sample_size(50);

    group.bench_function("encode", |b| {
        b.iter(|| {
            let mut writer = Cursor::new(Vec::with_capacity(total_bytes));
            encode_multiple(&mut writer, &packets);
            writer
        });
    });

    group.bench_function("decode", |b| {
        b.iter(|| {
            let mut reader = Cursor::new(&encoded);
            decode_multiple(&mut reader, NUM_PACKETS)
        });
    });

    group.finish();

    let data_path = Path::new("data/HG1_003014_20260829_102905.l0b.ddm");

    // Prepare the packets once, outside the timed encoding benchmark.
    let (packets, file_size) = {
        let file = File::open(data_path).unwrap();
        decode_all(&mut BufReader::new(file), NUM_PACKETS)
    };

    // Benchmark encoding those packets to a temporary file.
    let mut temp_file = tempfile::NamedTempFile::new().unwrap();

    let mut group = c.benchmark_group("l0b_disk");

    group.throughput(Throughput::ElementsAndBytes {
        elements: packets.len() as u64,
        bytes: file_size as u64,
    });
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(10));
    group.warm_up_time(std::time::Duration::from_secs(3));

    group.bench_function("encode", |b| {
        b.iter(|| {
            let file = temp_file.as_file_mut();
            file.set_len(0).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();

            {
                let mut writer = BufWriter::new(file);
                encode_multiple(&mut writer, &packets);
                writer.flush().unwrap();
            }
        });
    });

    group.bench_function("decode", |b| {
        b.iter(|| {
            let file = File::open(data_path).unwrap();
            let mut reader = BufReader::new(file);
            decode_all(&mut reader, NUM_PACKETS)
        });
    });

    group.finish()
}

criterion_group!(benches, l0b);
criterion_main!(benches);
