//! DDM processing app

mod compare;
mod encoding;
mod file;
mod filter;
mod frame_map;
mod inspect;
mod l0b;
mod pipeline;
mod error;

pub use compare::{AggregateComparisonResult, ComparisonResult, compare, compare_frames};
pub use encoding::{decode, encode, read, write};
pub use file::{L0bConversionResult, L0bFileData, convert_l0b, inspect_l0b, load_l0b};
pub use filter::{DDMFilters, filter, parse_ranges};
pub use frame_map::DDMFrameMap;
pub use inspect::inspect;
pub use l0b::{GpsSecondRange, L0bFileInfo, L0bFormat, L0bFormatConfig};
pub use pipeline::{PipelineChannel, fanout};
pub use error::{downcast_join_err, map_join_err};
