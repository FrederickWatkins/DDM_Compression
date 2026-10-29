//! DDM CLI tools

use std::io::{BufReader, Cursor, Seek, SeekFrom};

use binrw::{BinRead as _, Endian};
use bitstream_io::{BigEndian, BitWriter};
use ddm_core::{
    coding::{
        BitplaneConfig, BitplanePixelData, WaveletPixelData, bitplane,
        wavelet::{MallatSubband, WaveletSubband, dwt_53_nd},
    },
    l0b::{DDMDecoder, L0bPacket, MP00Decoder, mp00::encode_pixel_data},
};
use ndarray::Array2;

mod backtrace;

use backtrace::install_error_reporting as install_eyre_hook;

fn main() -> color_eyre::eyre::Result<()> {
    install_eyre_hook()?;
    let max_packets = 512;

    let config = BitplaneConfig {
        quantisation: bitplane::Quantisation::FixedQuantisation { bits: 0 },
        dc_encoding: bitplane::SignEncoding::TwosComplement { bypass: true },
        ac_encoding: bitplane::SignEncoding::Bitplanes { bypass_sign: true },
        significant_encoding: bitplane::SignificantEncoding::UseMainContext,
        neighbour_encoding: bitplane::NeighbourEncoding::UseOwnContext,
        refinement_encoding: bitplane::RefinementEncoding::Bypass,
    };

    let file = std::fs::File::open("data/L0b/HG1_003014_20260829_102905.l0b.ddm")?;
    let mut reader = BufReader::new(file);
    let mut decoder = MP00Decoder::new();
    let mut frames = vec![];
    let mut buf = vec![];
    let mut cursor = Cursor::new(&mut buf);
    let mut bitwriter = BitWriter::<_, BigEndian>::new(&mut cursor);
    let mut buf2 = vec![];
    let mut cursor2 = Cursor::new(&mut buf2);

    for _ in 0..max_packets {
        let current_pos = reader.stream_position()?;
        let end_pos = reader.seek(SeekFrom::End(0))?;

        if current_pos == end_pos {
            break;
        }
        reader.seek(SeekFrom::Start(current_pos))?;
        let packet = L0bPacket::read_options(&mut reader, Endian::Big, (52, 20))?;
        match packet {
            L0bPacket::MP00(mp00_packet) => {
                frames.push(decoder.decode(mp00_packet)?.pixel_data().to_owned())
            }
            L0bPacket::MT00(_) => (),
        }
    }

    let frame = &frames[211];

    encode_pixel_data(&mut cursor2, &frame.view())?;

    let mut all_values: Vec<_> = frames
        .iter()
        .flat_map(|frame| frame.iter().copied())
        .collect();

    if !all_values.is_empty() {
        // Find the index corresponding to the 99th percentile
        let idx = ((all_values.len() as f64) * 0.999).floor() as usize;

        // Use select_nth_unstable for O(N) linear-time selection without a full sort
        let (_, &mut p99, _) = all_values.select_nth_unstable(idx);

        println!("99th percentile pixel value: {}", p99);
    }

    println!("Encoded length (legacy): {}", buf2.len());

    let mut wavelet_pixels = WaveletPixelData::new(frame.mapv(i32::from));
    let mean = wavelet_pixels
        .get(MallatSubband {
            stage: 0,
            subband: WaveletSubband::LL,
        })
        .unwrap()
        .mean()
        .unwrap();
    // wavelet_pixels
    //     .get_mut(MallatSubband {
    //         stage: 0,
    //         subband: WaveletSubband::LL,
    //     })
    //     .unwrap()
    //     .map_inplace(|x| *x = *x - mean);
    wavelet_pixels.dwt_mallat(2)?;
    let bitplanes = BitplanePixelData::new(wavelet_pixels);
    bitplanes.encode(&mut bitwriter, config)?;
    println!("Encoded length (experimental): {}", buf.len());

    Ok(())
}
