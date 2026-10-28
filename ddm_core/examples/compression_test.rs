#![allow(
    warnings,
    clippy::all,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "Example code"
)]

use std::io::Cursor;

use bitstream_io::{BigEndian, BitReader, BitWrite as _, BitWriter};
use ddm_core::coding::{
    BitplaneConfig, BitplanePixelData, bitplane,
    wavelet::{WaveletPixelData, WaveletSubband},
};
use ndarray::Array2;
use rand::{Rng, RngExt as _, SeedableRng, rngs::StdRng};

fn main() {
    let mut rng = StdRng::seed_from_u64(42);
    let config = BitplaneConfig {
        quantisation: bitplane::Quantisation::AdaptiveQuantisation { base_bits: 1 },
        dc_encoding: bitplane::SignEncoding::TwosComplement { bypass: true },
        ac_encoding: bitplane::SignEncoding::Bitplanes { bypass_sign: true },
        significant_encoding: bitplane::SignificantEncoding::UseMainContext,
        neighbour_encoding: bitplane::NeighbourEncoding::UseOwnContext,
        refinement_encoding: bitplane::RefinementEncoding::UseOwnContext,
    };
    let raw_data = Array2::from_shape_fn((52, 20), |_| rng.random_range(3000..=3500));
    let mut wavelet_coded = WaveletPixelData::new(raw_data.clone());
    wavelet_coded.dwt_mallat(3);
    let no_quantisation = BitplanePixelData::new(wavelet_coded.clone());
    let dc_bitdepth = no_quantisation.dc_bitdepth_twos_complement();
    let ac_bitdepth = no_quantisation.ac_bitdepth_magnitude();
    let mut buf = vec![];
    let mut cursor = Cursor::new(&mut buf);
    let mut writer = BitWriter::<_, BigEndian>::new(&mut buf);
    no_quantisation.encode(&mut writer, config).unwrap();
    writer.byte_align();
    let mut cursor = Cursor::new(&mut buf);
    let mut reader = BitReader::<_, BigEndian>::new(&mut cursor);
    let mut round_trip = BitplanePixelData::decode(
        WaveletPixelData::empty((52, 20), 3),
        &mut reader,
        config,
        dc_bitdepth,
        ac_bitdepth,
    )
    .unwrap();
    let mut round_trip = round_trip.into_wavelet_data();
    wavelet_coded.idwt_mallat();
    round_trip.idwt_mallat();
    println!("Size in bytes {}", buf.len());
    assert_eq!(wavelet_coded, round_trip);
}
