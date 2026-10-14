//! L0b format packets
pub mod mp00;
pub mod mq00;
pub mod mt00;

use anyhow::{Result, anyhow};
use std::io::{Read, Seek, Write};

/// Trait for creating structs by decoding from a byte stream
pub trait Decode: Sized {
    /// Attempt to create a packet from input byte stream
    fn decode<R: Read + Seek>(reader: &mut R) -> Result<Self>;
}

/// Trait for writing structs into a byte stream
pub trait Encode {
    /// Attempt to write packet into output byte stream
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()>;
}

/// Magic number for all L0b packets
const MAGIC_PREAMBLE: u32 = 0xCA75_0200;

const MAGIC_MP00: u32 = u32::from_be_bytes(*b"MP00");
const MAGIC_MT00: u32 = u32::from_be_bytes(*b"MT00");
const MAGIC_MQ00: u32 = u32::from_be_bytes(*b"MQ00");

/// Wrapper enum for all L0b packet types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L0bPacket {
    /// MP00 packet type; see [`mp00`]
    MP00(Box<mp00::MP00>),
    /// MT00 packet type; see [`mt00`]
    MT00(Box<mt00::MT00>),
    /// MQ00 packet type; see [`mq00`]
    MQ00(Box<mq00::MQ00>),
}

impl Decode for L0bPacket {
    fn decode<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf)?;

        let packet_preamble = u32::from_be_bytes(buf[0..4].try_into().unwrap());
        let packet_type_raw = u32::from_be_bytes(buf[4..8].try_into().unwrap());

        if packet_preamble != MAGIC_PREAMBLE {
            return Err(anyhow!("Unknown preamble"));
        }

        Ok(match packet_type_raw {
            MAGIC_MP00 => L0bPacket::MP00(Box::new(mp00::MP00::decode(reader)?)),
            MAGIC_MT00 => L0bPacket::MT00(Box::new(mt00::MT00::decode(reader)?)),
            MAGIC_MQ00 => L0bPacket::MQ00(Box::new(mq00::MQ00::decode(reader)?)),
            _ => {
                return Err(anyhow!("Packet type not found"));
            }
        })
    }
}

impl Encode for L0bPacket {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<()> {
        let packet_type = match self {
            L0bPacket::MP00(_) => MAGIC_MP00,
            L0bPacket::MT00(_) => MAGIC_MT00,
            L0bPacket::MQ00(_) => MAGIC_MQ00,
        };

        writer.write_all(&MAGIC_PREAMBLE.to_be_bytes())?;
        writer.write_all(&packet_type.to_be_bytes())?;

        match self {
            L0bPacket::MP00(packet) => packet.encode(writer)?,
            L0bPacket::MT00(packet) => packet.encode(writer)?,
            L0bPacket::MQ00(packet) => packet.encode(writer)?,
        }

        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::l0b_packet::{mp00::tests::arb_mp00, mq00::tests::arb_mq00, mt00::tests::arb_mt00};
    use proptest::prelude::*;

    fn arb_l0b_packet(
        pixel_data_strategy: impl Strategy<Value = u16> + Clone,
    ) -> impl Strategy<Value = L0bPacket> {
        prop_oneof![
            arb_mp00(pixel_data_strategy).prop_map(|p| L0bPacket::MP00(Box::new(p))),
            arb_mt00().prop_map(|p| L0bPacket::MT00(Box::new(p))),
            // arb_mq00(pixel_data_strategy).prop_map(|p| L0bPacket::MQ00(Box::new(p))),
        ]
    }
    proptest! {
        #[test]
        fn l0b_roundtrip(
            packets in prop::collection::vec(arb_l0b_packet(any::<u16>()), 0..20)
        ) {
            let mut encoded = vec![];

            for packet in &packets {
                packet.encode(&mut encoded).unwrap();
            }

            let mut reader = Cursor::new(encoded);
            let mut decoded = Vec::new();

            while (reader.position() as usize) < reader.get_ref().len() {
                decoded.push(L0bPacket::decode(&mut reader).unwrap());
            }

            prop_assert_eq!(decoded, packets);
        }
    }
}
