//! Library error types

use crate::l0b::{EncodingError};

/// Library result type
pub type Result<T> = core::result::Result<T, Error>;

/// Library error types
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// [`binrw::io`] error
    #[error(transparent)]
    Io(#[from] binrw::io::Error),
    #[error("encoding error: {0}")]
    /// Error during encoding
    EncodingError(#[from] EncodingError),
}
