use std::io::{self, Read, Seek};

mod mp00;
mod mt00;

pub trait Decode: Sized {
    fn decode<R: Read + Seek>(reader: &mut R) -> Result<Self, std::io::Error>;
}

const MAGIC_MP00: u32 = u32::from_be_bytes(*b"MP00");
const MAGIC_MT00: u32 = u32::from_be_bytes(*b"MT00");

#[derive(Debug, Clone, PartialEq)]
pub enum L0bPacketType {
    Mp00(mp00::Mp00),
    Mt00(mt00::Mt00),
}

pub struct L0bPacket {
    pub packet_preamble: u32,
    pub packet_type: L0bPacketType,
}

impl Decode for L0bPacket {
    fn decode<R: Read + Seek>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf)?;

        let packet_preamble = u32::from_be_bytes(buf[0..4].try_into().unwrap());
        let packet_type_raw = u32::from_be_bytes(buf[4..8].try_into().unwrap());

        if packet_preamble != 0xCA750200 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Packet preamble does not match",
            ));
        }

        let packet_type = match packet_type_raw {
            MAGIC_MP00 => L0bPacketType::Mp00(mp00::Mp00::decode(reader)?),
            MAGIC_MT00 => L0bPacketType::Mt00(mt00::Mt00::decode(reader)?),
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Packet type not known",
                ));
            }
        };

        Ok(L0bPacket {
            packet_preamble,
            packet_type,
        })
    }
}
