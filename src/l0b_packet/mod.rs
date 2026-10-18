//! L0b format packets
pub mod mp00;
pub mod mq00;
pub mod mt00;

use anyhow::{Error, Result, anyhow};
use std::io::{self, Read, Seek, Write};

/// Trait for creating structs by decoding from a byte stream
pub trait Decode: Sized {
    /// Attempt to create a packet from input byte stream
    fn decode<R: Read + Seek>(reader: &mut R) -> Result<Self>;
}

/// Trait for writing structs into a byte stream
pub trait Encode {
    /// Attempt to write packet into output byte stream
    fn encode<W: Write + Seek>(&self, writer: &mut W) -> Result<()>;
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

        let packet_preamble = u32::from_be_bytes(buf[0..4].try_into()?);
        let packet_type_raw = u32::from_be_bytes(buf[4..8].try_into()?);

        if packet_preamble != MAGIC_PREAMBLE {
            return Err(anyhow!(
                "Unknown L0B packet preamble {packet_preamble:#010x}"
            ));
        }

        Ok(match packet_type_raw {
            MAGIC_MP00 => L0bPacket::MP00(Box::new(mp00::MP00::decode(reader)?)),
            MAGIC_MT00 => L0bPacket::MT00(Box::new(mt00::MT00::decode(reader)?)),
            MAGIC_MQ00 => L0bPacket::MQ00(Box::new(mq00::MQ00::decode(reader)?)),
            _ => {
                return Err(anyhow!("Unknown L0B packet type {packet_type_raw:#010x}"));
            }
        })
    }
}

impl Encode for L0bPacket {
    fn encode<W: Write + Seek>(&self, writer: &mut W) -> Result<()> {
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

/// Decode packets from a stream until clean end-of-file or the first malformed packet.
///
/// An incomplete packet is returned as an error. Only end-of-file before any bytes of the next
/// packet is treated as normal completion.
///
/// # Examples
///
/// ```
/// use ddm_compression::l0b_packet::decode_all;
/// use std::io::Cursor;
///
/// let mut reader = Cursor::new(Vec::<u8>::new());
/// let packets = decode_all(&mut reader).collect::<Result<Vec<_>, _>>()?;
/// assert!(packets.is_empty());
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn decode_all<R: Read + Seek>(reader: &mut R) -> impl Iterator<Item = Result<L0bPacket>> {
    fn is_unexpected_eof(err: &Error) -> bool {
        match err.downcast_ref::<io::Error>() {
            Some(e) => e.kind() == io::ErrorKind::UnexpectedEof,
            None => false,
        }
    }
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }

        let start = match reader.stream_position() {
            Ok(position) => position,
            Err(error) => {
                done = true;
                return Some(Err(error.into()));
            }
        };

        match L0bPacket::decode(reader) {
            Ok(packet) => Some(Ok(packet)),
            Err(error) if is_unexpected_eof(&error) => match reader.stream_position() {
                Ok(position) if position == start => {
                    done = true;
                    None
                }
                Ok(_) => {
                    done = true;
                    Some(Err(error))
                }
                Err(position_error) => {
                    done = true;
                    Some(Err(position_error.into()))
                }
            },
            Err(error) => {
                done = true;
                Some(Err(error))
            }
        }
    })
}

/// Encode a sequence of L0B packets to a seekable output stream.
///
/// # Errors
///
/// Returns an error if writing any packet to `writer` fails.
pub fn encode_all<W: Write + Seek>(packets: &[L0bPacket], writer: &mut W) -> Result<()> {
    for packet in packets {
        packet.encode(writer)?;
    }
    Ok(())
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
            arb_mp00(pixel_data_strategy.clone()).prop_map(|p| L0bPacket::MP00(Box::new(p))),
            arb_mt00().prop_map(|p| L0bPacket::MT00(Box::new(p))),
            arb_mq00(pixel_data_strategy.clone()).prop_map(|p| L0bPacket::MQ00(Box::new(p))),
        ]
    }
    proptest! {
        #[test]
        fn l0b_roundtrip(
            packets in prop::collection::vec(arb_l0b_packet(any::<u16>()), 0..20)
        ) {
            let mut cursor = Cursor::new(Vec::new());

            for packet in &packets {
                packet.encode(&mut cursor).unwrap();
            }

            let encoded = cursor.into_inner();

            let mut reader = Cursor::new(encoded);
            let decoded = decode_all(&mut reader).collect::<Result<Vec<_>>>();
            match decoded {
                Ok(decoded) => prop_assert_eq!(decoded, packets),
                Err(error) => prop_assert!(false, "packet stream failed to decode: {error}"),
            }
        }
    }
}
