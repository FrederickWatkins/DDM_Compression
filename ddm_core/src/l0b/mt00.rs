//! MT00 calibration packet

use binrw::{BinRead, BinWrite};
#[cfg(feature = "serde")]
use serde_big_array::BigArray;

/// MT00 packet type
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[brw(big)]
pub struct MT00Packet {
    #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
    buf: [u8; 56],
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::vec;

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
            for _ in 0..packets.len() {
                decoded_packets.push(MT00Packet::read(&mut cursor)?);
            }
            prop_assert_eq!(packets, decoded_packets);
        }
    }
}
