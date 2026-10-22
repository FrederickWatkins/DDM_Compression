//! DDM processing app

mod compare;
mod encoding;
mod filter;
mod frame_map;
mod inspect;
mod l0b;
mod pipeline;

pub use compare::{AggregateComparisonResult, ComparisonResult, compare, compare_frames};
pub use encoding::{decode, encode, read, write};
pub use filter::{DDMFilters, filter, parse_ranges};
pub use frame_map::DDMFrameMap;
pub use inspect::inspect;
pub use l0b::{L0bFileInfo, L0bFormat, L0bFormatConfig};
pub use pipeline::fanout;
