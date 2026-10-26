//! Command-line interface for DDM processing.

mod args;
mod benchmark;
mod compare;
mod convert;
mod inspect;
mod paths;
mod table;

pub use args::{Cli, Subcommands, exit_clap_error};
pub use benchmark::benchmark;
pub use compare::compare;
pub use convert::convert;
pub use inspect::inspect;
pub use table::{ComparisonInfoEntry, FileInfoEntry, format_comparison_info, format_file_info};

const WIDTH: usize = 20;
const HEIGHT: usize = 52;
type L0bFileData = ddm_app::L0bFileData<WIDTH, HEIGHT>;
type DDMFrame = ddm_core::l0b::DDMFrame<WIDTH, HEIGHT>;
type L0bPacket = ddm_core::l0b::L0bPacket<WIDTH, HEIGHT>;
