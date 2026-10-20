//! L0b app integration test using reference data
use std::{
    io::{BufRead as _, Cursor},
    path::Path,
};

use binrw::BinRead as _;
use ddm_app::{DDMFilters, L0bFormat, L0bFormatConfig, convert, inspect, parse_ranges};
use ddm_core::l0b::L0bPacket;
use prodash::progress::Discard;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

#[test]
fn convert_real_data_and_preserve_packets() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let input = Path::new(DATA_DIR).join("HG1_TEST_512.l0b.ddm");
    let mq00_output = temp_dir.path().join("converted-mq00.l0b.ddm");
    let mp00_output = temp_dir.path().join("converted-mp00.l0b.ddm");
    let filters = DDMFilters {
        channel: parse_ranges("")?,
        gps_seconds: parse_ranges("")?,
    };
    let mut progress = Discard;

    let input_info = inspect::<20, 52>(input.clone(), &mut progress)?;
    assert!(input_info.num_packets() > 0);
    assert_eq!(
        input_info.formats(),
        &std::collections::HashSet::from([L0bFormat::MP00])
    );

    convert::<20, 52>(
        input.clone(),
        mq00_output.clone(),
        &filters,
        L0bFormatConfig::MQ00 {
            quantisation: 0,
            dc_adjust: false,
            wavelet_stages: 4,
            gop_length: 30,
        },
        &mut progress,
    )?;
    let mq00_info = inspect::<20, 52>(mq00_output.clone(), &mut progress)?;
    assert_eq!(mq00_info.num_frames(), input_info.num_frames());
    assert_eq!(
        mq00_info.formats(),
        &std::collections::HashSet::from([L0bFormat::MQ00])
    );

    convert::<20, 52>(
        mq00_output,
        mp00_output.clone(),
        &filters,
        L0bFormatConfig::MP00 { quantisation: 0 },
        &mut progress,
    )?;

    let input_frames = read_packets(input)?
        .into_iter()
        .filter(|packet| !matches!(packet, L0bPacket::MT00(_)))
        .collect::<Vec<_>>();
    let output_frames = read_packets(mp00_output)?;
    assert_eq!(input_frames, output_frames);

    Ok(())
}

fn read_packets(
    path: impl AsRef<Path>,
) -> Result<Vec<L0bPacket<20, 52>>, Box<dyn std::error::Error>> {
    let data = std::fs::read(path)?;
    let mut cursor = Cursor::new(data);
    let mut packets = Vec::new();
    while !cursor.fill_buf()?.is_empty() {
        packets.push(L0bPacket::<20, 52>::read(&mut cursor)?);
    }
    Ok(packets)
}
