//! Command-line interface for DDM processing.

mod args;
mod compare;
mod convert;
mod inspect;
mod paths;
mod table;

pub use args::{Cli, HEIGHT, Subcommands, WIDTH, exit_clap_error};
pub use compare::compare;
pub use convert::convert;
pub use inspect::inspect;
pub use table::{ComparisonInfoEntry, FileInfoEntry, format_comparison_info, format_file_info};
