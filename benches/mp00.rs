use std::io::{Cursor, Read, Seek, Write};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ddm_compression::{
    DDMPixelData,
    l0b_packet::{Decode, Encode, mp00},
};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

const NUM_PACKETS: usize = 100;

fn encode_multiple<W: Write>(writer: &mut W, packets: &Vec<mp00::MP00>) {
    for packet in packets {
        packet.encode(writer).unwrap();
    }
}

fn decode_multiple<R: Read + Seek>(reader: &mut R, count: usize) -> Vec<mp00::MP00> {
    (0..count)
        .map(|_| mp00::MP00::decode(reader).unwrap())
        .collect()
}

fn make_packets(num_packets: usize) -> Vec<mp00::MP00> {
    let mut rng = StdRng::seed_from_u64(42);

    (0..num_packets)
        .map(|_| {
            let mut pixel_data: DDMPixelData = [[0; _]; _];

            for row in &mut pixel_data {
                for pixel in row {
                    *pixel = rng.random_range(1000..1200);
                }
            }

            mp00::MP00::new(1, 0, 0, 6332524, 2354353, pixel_data)
        })
        .collect()
}

pub fn mp00_encode(c: &mut Criterion) {
    let packets = make_packets(NUM_PACKETS);

    let mut buffer = Vec::new();
    encode_multiple(&mut buffer, &packets);
    let total_bytes = buffer.len();

    let mut group = c.benchmark_group("mp00_encode");

    group.throughput(Throughput::ElementsAndBytes {
        elements: packets.len() as u64,
        bytes: total_bytes as u64,
    });
    group.bench_function("MP00_encode", |b| {
        b.iter(|| {
            let mut writer = Vec::with_capacity(total_bytes);
            encode_multiple(&mut writer, &packets);
            writer
        });
    });

    group.finish();
}

pub fn mp00_decode(c: &mut Criterion) {
    let packets = make_packets(NUM_PACKETS);

    // Encode once outside the timed benchmark.
    let mut encoded = Vec::new();
    encode_multiple(&mut encoded, &packets);
    let total_bytes = encoded.len();

    let mut group = c.benchmark_group("mp00_decode");

    group.throughput(Throughput::ElementsAndBytes {
        elements: packets.len() as u64,
        bytes: total_bytes as u64,
    });
    group.bench_function("MP00_decode", |b| {
        b.iter(|| {
            let mut reader = Cursor::new(&encoded);
            decode_multiple(&mut reader, NUM_PACKETS)
        });
    });

    group.finish();
}

criterion_group!(benches, mp00_encode, mp00_decode);
criterion_main!(benches);
