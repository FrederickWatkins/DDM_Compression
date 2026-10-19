//! L0b data integration test
use std::{
    fs::File, io::{BufRead as _, BufReader, Cursor, Read as _}, path::Path,
};

use anyhow::anyhow;
use binrw::{BinRead as _, BinWrite as _};
use ddm_core::l0b::{
    DDMDecoder as _, DDMEncoder as _, L0bPacket, MP00Decoder, MP00Encoder, MQ00Decoder,
    MQ00Encoder, MQ00EncoderConfig,
};

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");

#[test]
fn mp00_mq00_round_trip() -> anyhow::Result<()> {
    let l0b_path = Path::new(DATA_DIR).join("HG1_003014_20260829_102905.l0b.ddm");
    let l0b_data = File::open(l0b_path)?;
    let mut reader = BufReader::new(&l0b_data);
    let mut decoded_packets = vec![];
    loop {
        if reader.fill_buf()?.is_empty() {
            break;
        }

        decoded_packets.push(L0bPacket::<20, 52>::read(&mut reader)?);
    }
    let mq00_config = MQ00EncoderConfig::new(0, true, 4, 30)?;
    let mut mq00_encoder = MQ00Encoder::new(mq00_config);
    let mut mp00_decoder = MP00Decoder::new();
    let mq00_packets = decoded_packets
        .into_iter()
        .map(|packet| {
            Ok(match packet {
                L0bPacket::MP00(mp00_packet) => {
                    L0bPacket::MQ00(mq00_encoder.encode(&mp00_decoder.decode(mp00_packet)?)?)
                }
                L0bPacket::MQ00(_) => return Err(anyhow!("MQ00 present in reference L0b data")),
                L0bPacket::MT00(mt00_packet) => L0bPacket::MT00(mt00_packet),
            })
        })
        .collect::<anyhow::Result<Vec<L0bPacket<20, 52>>>>()?;

    let mut buf = vec![];
    let mut cursor = Cursor::new(&mut buf);
    for packet in mq00_packets {
        packet.write(&mut cursor)?;
    }
    let mut decoded_packets = vec![];
    loop {
        if reader.fill_buf()?.is_empty() {
            break;
        }

        decoded_packets.push(L0bPacket::<20, 52>::read(&mut reader)?);
    }
    let mut mq00_decoder = MQ00Decoder::new();
    let mut mp00_encoder = MP00Encoder::new(0);
    let mp00_packets = decoded_packets
        .into_iter()
        .map(|packet| {
            Ok(match packet {
                L0bPacket::MP00(_) => return Err(anyhow!("MP00 present in reference L0b data")),
                L0bPacket::MQ00(mq00_packet) => {
                    L0bPacket::MP00(mp00_encoder.encode(&mq00_decoder.decode(mq00_packet)?)?)
                }
                L0bPacket::MT00(mt00_packet) => L0bPacket::MT00(mt00_packet),
            })
        })
        .collect::<anyhow::Result<Vec<L0bPacket<20, 52>>>>()?;

    let mut buf = vec![];
    let mut cursor = Cursor::new(&mut buf);
    for packet in mp00_packets {
        packet.write(&mut cursor)?;
    }

    assert_eq!(l0b_data.bytes().collect::<std::io::Result<Vec<u8>>>()?, buf);

    Ok(())
}
