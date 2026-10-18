//! Library error types

/// Library result type
pub type Result<T> = std::result::Result<T, ErrorKind>;

/// Library error types
#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    /// Illegal value error
    #[error("{name} must be between {min} and {max}, got {actual}")]
    ValueOutOfRange {
        /// Variable name
        name: &'static str,
        /// Minimum allowed value
        min: u64,
        /// Maximum allowed value
        max: u64,
        /// Actual value
        actual: u64,
    },
    /// Io error
    #[error(transparent)]
    Io(#[from] std::io::Error),
}