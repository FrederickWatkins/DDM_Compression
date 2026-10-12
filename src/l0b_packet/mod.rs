pub mod mc00;
pub mod mp00;
pub mod mt00;

use std::io::{Read, Seek, Write};
use anyhow::{Result, anyhow};

pub trait Decode: Sized {
    fn decode<R: Read + Seek>(reader: &mut R) -> Result<Self>;
}

pub trait Encode {
    fn encode<W: Write>(self, writer: &mut W) -> Result<()>;
}

const MAGIC_PREAMBLE: u32 = 0xCA750200;

const MAGIC_MP00: u32 = u32::from_be_bytes(*b"MP00");
const MAGIC_MT00: u32 = u32::from_be_bytes(*b"MT00");
const MAGIC_MC00: u32 = u32::from_be_bytes(*b"MC00");

pub enum L0bPacketType {
    Mp00(mp00::Mp00),
    Mt00(mt00::Mt00),
    Mc00(mc00::Mc00),
}

pub struct L0bPacket {
    pub packet_preamble: u32,
    pub packet_type: L0bPacketType,
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

        let packet_type = match packet_type_raw {
            MAGIC_MP00 => L0bPacketType::Mp00(mp00::Mp00::decode(reader)?),
            MAGIC_MT00 => L0bPacketType::Mt00(mt00::Mt00::decode(reader)?),
            MAGIC_MC00 => L0bPacketType::Mc00(mc00::Mc00::decode(reader)?),
            _ => {
                return Err(anyhow!("Packet type not found"));
            }
        };

        Ok(L0bPacket {
            packet_preamble,
            packet_type,
        })
    }
}
