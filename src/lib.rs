//! Experimental crate for DDM compression research
#![warn(missing_docs)]

pub mod app;
pub mod convert;
pub mod l0b_packet;

/// Number of delay samples in a DDM.
pub const DDM_HEIGHT: usize = 52;
/// Number of doppler samples in a DDM.
pub const DDM_WIDTH: usize = 20;
/// Number of hardware channels in a DDM.
pub const DDM_CHANNEL_COUNT: u8 = 16;

/// Useful helper type for static sized pixel data
pub type DDMPixelData = [[u16; DDM_WIDTH]; DDM_HEIGHT];

#[cfg(test)]
mod tests {
    use proptest::strategy::Strategy;

    use super::{DDM_HEIGHT, DDM_WIDTH, DDMPixelData};

    // Custom strategy to generate DDMPixelData.
    // Adjust the dimensions (e.g., 16x16) to match whatever DDMPixelData is configured as in your
    // crate.
    pub fn arb_pixel_data(
        inner_strategy: impl Strategy<Value = u16> + Clone,
    ) -> impl Strategy<Value = DDMPixelData> {
        proptest::collection::vec(
            proptest::collection::vec(inner_strategy, DDM_WIDTH),
            DDM_HEIGHT,
        )
        .prop_map(|v| {
            v.into_iter()
                .map(|row| row.try_into().expect("vector length must match DDM_WIDTH"))
                .collect::<Vec<_>>()
                .try_into()
                .expect("vector length must match DDM_HEIGHT")
        })
    }
}
