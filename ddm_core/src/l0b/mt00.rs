//! MT00 calibration packet

use binrw::{BinRead, BinWrite};

/// MT00 packet type
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[brw(big)]
pub struct MT00Packet {
    buf: [u8; 56],
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Cursor;

    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn bytestream_round_trip(packets in proptest::collection::vec(any::<MT00Packet>(), 1..10)) {
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for packet in &packets {
                packet.write(&mut cursor)?;
            }
            let mut decoded_packets = vec![];
            let mut cursor = Cursor::new(cursor.into_inner());
            loop {
                match MT00Packet::read(&mut cursor) {
                    Ok(packet) => decoded_packets.push(packet),
                    Err(binrw::Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.into())
                }
            }
            prop_assert_eq!(packets, decoded_packets);
        }
    }
}
