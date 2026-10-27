//! L0b DDM Formats

use snafu::{Backtrace, Snafu};

use crate::coding::{cabac, wavelet};

/// DDM encoding error types
#[derive(Debug, Snafu)]
pub enum Error {
    /// Error during wavelet coding
    #[snafu(transparent)]
    Wavelet {
        /// Error source
        source: wavelet::Error,
    },
    /// Error during cabac compression
    #[snafu(transparent)]
    Cabac {
        /// Error source
        source: cabac::Error,
    },
    /// Error from binrw
    #[snafu(transparent)]
    Binrw {
        /// Error source
        source: binrw::Error,
        /// Error backtrace
        backtrace: Backtrace,
    },
}
