//! Experimental MQ00 packet type
use crate::{
    DDMPixelData,
    l0b_packet::{Decode, Encode},
};

use anyhow::{Result, anyhow};
use ndarray::{Array2, ArrayView2, Axis};

pub mod cabac;

/// Experimental MQ00 packet type
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MQ00 {
    channel_num: u8,
    status_flag: u8,
    ll_quantisation: u8,
    detail_quantisation: u8,
    timestamp_seconds: u32,
    timestamp_clocks: i32,
    pixel_data: Array2<i32>,
}

impl Encode for MQ00 {
    fn encode<W: std::io::prelude::Write>(&self, writer: &mut W) -> anyhow::Result<()> {
        todo!()
    }
}

impl Decode for MQ00 {
    fn decode<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
    ) -> anyhow::Result<Self> {
        todo!()
    }
}

impl MQ00 {
    /// Create new MP00 packet from raw attributes
    ///
    /// `ll_quantisation` and `detail_quantisation` must be representable in 4 bits each
    pub fn new(
        channel_num: u8,
        status_flag: u8,
        ll_quantisation: u8,
        detail_quantisation: u8,
        timestamp_seconds: u32,
        timestamp_clocks: i32,
        pixel_data: DDMPixelData,
    ) -> Result<Self> {
        if ll_quantisation > 0xF {
            return Err(anyhow!(
                "Illegal ll_quantisation {ll_quantisation}; low frequency quantisation must be representable in 4 bits"
            ));
        }
        if detail_quantisation > 0xF {
            return Err(anyhow!(
                "Illegal detail_quantisation {detail_quantisation}; high frequency quantisation must be representable in 4 bits"
            ));
        }
        let shape = (pixel_data.len(), pixel_data[0].len());
        let flat = pixel_data.as_flattened();

        let pixel_data = ArrayView2::from_shape(shape, flat)?.mapv(i32::from);
        Ok(Self {
            channel_num,
            status_flag,
            ll_quantisation,
            detail_quantisation,
            timestamp_seconds,
            timestamp_clocks,
            pixel_data,
        })
    }

    /// DDM hardware channel number
    pub fn channel_num(&self) -> u8 {
        self.channel_num
    }

    /// Time stamp of integration start: GPS seconds at last PPS as rounded integer
    pub fn timestamp_seconds(&self) -> u32 {
        self.timestamp_seconds
    }

    /// Time stamp of integration start: sample clocks since last PPS
    pub fn timestamp_clocks(&self) -> i32 {
        self.timestamp_clocks
    }

    /// DDM pixel data
    pub fn pixel_data(&self) -> Result<DDMPixelData> {
        let mut output: DDMPixelData = [[0; _]; _];
        for (new_row, old_row) in output.iter_mut().zip(self.pixel_data.axis_iter(Axis(0))) {
            for (new_pixel, old_pixel) in new_row.iter_mut().zip(old_row.iter()) {
                *new_pixel = u16::try_from(*old_pixel)?;
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::tests::arb_pixel_data;

    use proptest::prelude::*;

    // Custom strategy to generate arbitrary valid MP00 instances
    pub fn arb_mq00(
        pixel_data_strategy: impl Strategy<Value = u16> + Clone,
    ) -> impl Strategy<Value = MQ00> {
        (
            any::<u8>(),
            any::<u8>(),
            0..16u8,
            0..16u8,
            any::<u32>(),
            any::<i32>(),
            arb_pixel_data(pixel_data_strategy),
        )
            .prop_map(
                |(
                    channel_num,
                    status_flag,
                    ll_quantisation,
                    detail_quantisation,
                    timestamp_seconds,
                    timestamp_clocks,
                    pixel_data,
                )| {
                    MQ00::new(
                        channel_num,
                        status_flag,
                        ll_quantisation,
                        detail_quantisation,
                        timestamp_seconds,
                        timestamp_clocks,
                        pixel_data,
                    )
                    .unwrap()
                },
            )
    }

    proptest! {
        #[test]
        fn pixel_conversion(original in arb_pixel_data(any::<u16>())) {
            // Encode the packet into an in-memory buffer
            let packet = MQ00::new(0, 0, 0, 0, 0, 0, original).unwrap();
            // Ensure round-trip integrity holds
            prop_assert_eq!(original, packet.pixel_data().unwrap());
        }

        #[test]
        fn quantisation_limit(ll_quantisation in 16u8.., detail_quantisation in 16u8..) {
            let packet = MQ00::new(0, 0, ll_quantisation, detail_quantisation, 0, 0, [[0; 20]; 52]);

            prop_assert!(packet.is_err());
        }
    }
}
