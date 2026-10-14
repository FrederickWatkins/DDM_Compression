use std::io::{Cursor, Read, Seek, Write};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ddm_compression::l0b_packet::{Decode, Encode, mt00};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

const NUM_PACKETS: usize = 100;

fn encode_multiple<W: Write>(writer: &mut W, packets: &Vec<mt00::MT00>) {
    for packet in packets {
        packet.encode(writer).unwrap();
    }
}

fn decode_multiple<R: Read + Seek>(reader: &mut R, count: usize) -> Vec<mt00::MT00> {
    (0..count)
        .map(|_| mt00::MT00::decode(reader).unwrap())
        .collect()
}

fn make_packets(num_packets: usize) -> Vec<mt00::MT00> {
    let mut rng = StdRng::seed_from_u64(42);

    (0..num_packets)
        .map(|_| {
            let mut buf = [0u8; 56];

            for byte in &mut buf {
                *byte = rng.random();
            }

            mt00::MT00::new(&buf)
        })
        .collect()
}

pub fn mt00(c: &mut Criterion) {
    let packets = make_packets(NUM_PACKETS);

    // Encode once outside the timed benchmarks.
    let mut encoded = Vec::new();
    encode_multiple(&mut encoded, &packets);
    let total_bytes = encoded.len();

    let mut group = c.benchmark_group("mt00");

    group.throughput(Throughput::ElementsAndBytes {
        elements: packets.len() as u64,
        bytes: total_bytes as u64,
    });

    group.bench_function("encode", |b| {
        b.iter(|| {
            let mut writer = Vec::with_capacity(total_bytes);
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
}

criterion_group!(benches, mt00);
criterion_main!(benches);
