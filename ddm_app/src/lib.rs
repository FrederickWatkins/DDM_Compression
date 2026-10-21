//! DDM file manipulation and analysis.

pub mod comparison;
pub mod conversion;
pub mod encoding;
pub mod file;
pub mod filters;
pub mod format;
pub mod inspection;

pub use comparison::{DDMComparisonInfo, compare};
pub use conversion::convert;
pub use filters::{DDMFilters, parse_ranges};
pub use format::{DDMFormat, L0bFormat, L0bFormatConfig};
pub use inspection::{L0bFileInfo, inspect};

/// DDM frame dimensions used by the input dataset.
pub const WIDTH: usize = 20;
/// DDM frame dimensions used by the input dataset.
pub const HEIGHT: usize = 52;
