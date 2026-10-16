use std::io::Cursor;

use anyhow::{Result, anyhow};
use ddm_compression::{
    DDMPixelData,
    convert::{into_mq00_array, into_raw_array},
    l0b_packet::{Encode, L0bPacket, decode_all, mp00::MP00, mq00::MQ00, mt00::MT00},
};

fn pixels(value: u16) -> DDMPixelData {
    [[value; 20]; 52]
}

#[test]
fn packet_stream_round_trips_and_reports_truncated_packets() -> Result<()> {
    let packets = vec![
        L0bPacket::MP00(Box::new(MP00::new(1, 0, 0, 100, 0, pixels(42)))),
        L0bPacket::MQ00(Box::new(MQ00::new(2, 0, 0, 0, 101, 0, pixels(84))?)),
        L0bPacket::MT00(Box::new(MT00::new(&[7; 56]))),
    ];

    let mut writer = Cursor::new(Vec::new());
    for packet in &packets {
        packet.encode(&mut writer)?;
    }
    let encoded = writer.into_inner();

    let decoded = decode_all(&mut Cursor::new(&encoded)).collect::<Result<Vec<_>>>()?;
    assert_eq!(decoded, packets);

    let mut truncated = encoded;
    truncated.pop();
    let mut truncated_reader = Cursor::new(truncated);
    let mut decoded = decode_all(&mut truncated_reader);
    assert!(decoded.next().is_some_and(|packet| packet.is_ok()));
    assert!(decoded.next().is_some_and(|packet| packet.is_ok()));
    assert!(decoded.next().is_some_and(|packet| packet.is_err()));
    assert!(decoded.next().is_none());
    Ok(())
}

#[test]
fn conversion_builds_a_sparse_timeline_and_preserves_pixels() -> Result<()> {
    let packets = vec![
        L0bPacket::MP00(Box::new(MP00::new(1, 0, 0, 100, 0, pixels(42)))),
        L0bPacket::MQ00(Box::new(MQ00::new(2, 0, 0, 0, 102, 0, pixels(84))?)),
        L0bPacket::MT00(Box::new(MT00::new(&[0; 56]))),
    ];

    let timeline = into_mq00_array(packets)?;
    assert_eq!(timeline.dim(), (16, 3));
    let first_packet = timeline[[1, 0]]
        .as_ref()
        .ok_or_else(|| anyhow!("converted MP00 packet is missing"))?;
    assert_eq!(first_packet.pixel_data()?[0][0], 42);
    assert!(timeline[[1, 1]].is_none());
    let last_packet = timeline[[2, 2]]
        .as_ref()
        .ok_or_else(|| anyhow!("MQ00 packet is missing"))?;
    assert_eq!(last_packet.pixel_data()?[0][0], 84);

    let raw = into_raw_array(&timeline.view())?;
    assert_eq!(raw.dim(), (16, 3, 52, 20));
    assert_eq!(raw[[1, 0, 0, 0]], Some(42));
    assert_eq!(raw[[2, 2, 0, 0]], Some(84));
    Ok(())
}

#[test]
fn conversion_rejects_channels_outside_the_ddm_range() {
    let packets = vec![L0bPacket::MP00(Box::new(MP00::new(
        16,
        0,
        0,
        100,
        0,
        pixels(42),
    )))];

    assert!(into_mq00_array(packets).is_err());
}

#[test]
fn empty_packet_stream_is_a_clean_end_of_file() {
    assert!(
        decode_all(&mut Cursor::new(Vec::<u8>::new()))
            .next()
            .is_none()
    );
}
