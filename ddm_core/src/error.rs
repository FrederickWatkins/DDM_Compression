//! Library error types

use crate::l0b::EncodingError;

/// Trait for unspecified errors
pub trait CustomError: core::fmt::Display + core::fmt::Debug + Send + Sync {}

/// Library result type
pub type Result<T> = core::result::Result<T, Error>;

/// Library error types
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// [`binrw::io`] error
    #[error(transparent)]
    Io(#[from] binrw::io::Error),
    /// Error during encoding
    #[error("encoding error: {0}")]
    EncodingError(#[from] EncodingError),
    /// Custom boxed error
    #[error(transparent)]
    Binrw(binrw::Error),
}

impl From<binrw::Error> for Error {
    fn from(value: binrw::Error) -> Self {
        match value {
            binrw::Error::Io(error) => Self::Io(error),
            error => Self::Binrw(error),
        }
    }
}
