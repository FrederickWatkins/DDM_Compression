use std::path::PathBuf;

use clap::{
    Args, CommandFactory as _, Parser, Subcommand, ValueEnum,
    error::{ContextKind, ContextValue, ErrorKind},
    value_parser,
};
use ddm_app::{DDMFilters, L0bFormat, L0bFormatConfig};
use tracing::level_filters::LevelFilter;

/// DDM width (delay)
pub const WIDTH: usize = 20;
/// DDM height (doppler)
pub const HEIGHT: usize = 52;

/// Top level args
#[derive(Parser)]
pub struct Cli {
    /// Command to run
    #[command(subcommand)]
    pub command: Subcommands,
    /// Logging level for stderr
    #[arg(long, short, default_value = "warn")]
    pub log_level: LogLevel,
    /// Suppress loading bars and non-essential output
    #[arg(long, short)]
    pub quiet: bool,
}

/// Subcommands
#[derive(Subcommand)]
pub enum Subcommands {
    /// View DDM metadata
    Inspect {
        /// Input .l0b.ddm file
        #[arg(required = true, num_args = 1..)]
        input: Vec<PathBuf>,
    },
    /// Filter DDM frames and write filtered output in chosen format
    Convert {
        /// .l0b.ddm file to read from
        #[arg(long, short)]
        input: PathBuf,
        /// .l0b.ddm file to write to
        #[arg(long, short)]
        output: PathBuf,
        /// Filters to apply to DDMs
        #[command(flatten)]
        filters: DDMFilters,
        /// DDM encoding format
        #[command(flatten)]
        format: L0bFormatArgs,
    },
    /// Compare DDM pixels and file metadata
    Compare {
        /// Two or more L0b series or Postcard frame files; each is compared with the first
        #[arg(required = true, num_args = 2..)]
        files: Vec<PathBuf>,
    },
}

#[derive(Debug, Args)]
#[command(next_help_heading = "Format options")]
pub struct L0bFormatArgs {
    /// Format to encode as
    #[arg(long, short = 'f', value_enum, default_value = "mp00")]
    pub format: L0bFormat,

    /// Quantisation
    #[arg(long, short)]
    pub quantisation: u8,

    /// Disable DC adjustment (MQ00 only)
    #[arg(long)]
    pub no_dc_adjust: bool,

    /// Number of wavelet transform stages (MQ00 only)
    #[arg(long, value_parser = value_parser!(u8).range(0..16))]
    pub wavelet_stages: Option<u8>,

    /// Length of a group of pictures (MQ00 only)
    #[arg(long)]
    pub gop_length: Option<u8>,
}

impl TryInto<L0bFormatConfig> for L0bFormatArgs {
    type Error = clap::Error;

    fn try_into(self) -> Result<L0bFormatConfig, Self::Error> {
        match self.format {
            L0bFormat::MP00 => {
                let mut conflict = Vec::new();
                if self.no_dc_adjust {
                    conflict.push("--no-dc-adjust".to_owned());
                }
                if self.wavelet_stages.is_some() {
                    conflict.push("--wavelet-stages <WAVELET_STAGES>".to_owned());
                }
                if self.gop_length.is_some() {
                    conflict.push("--gop-length <GOP_LENGTH>".to_owned());
                }
                if conflict.is_empty() {
                    Ok(L0bFormatConfig::MP00 {
                        quantisation: self.quantisation,
                    })
                } else {
                    let mut error = clap::Error::new(ErrorKind::ArgumentConflict);
                    error.insert(
                        ContextKind::InvalidArg,
                        ContextValue::String("--format mp00".to_owned()),
                    );
                    error.insert(ContextKind::PriorArg, ContextValue::Strings(conflict));
                    Err(error)
                }
            }
            L0bFormat::MQ00 => {
                let mut missing = Vec::new();
                let Some(wavelet_stages) = self.wavelet_stages else {
                    missing.push("--wavelet-stages <WAVELET_STAGES>".to_owned());
                    if self.gop_length.is_none() {
                        missing.push("--gop-length <GOP_LENGTH>".to_owned());
                    }
                    let mut error = clap::Error::new(ErrorKind::MissingRequiredArgument);
                    error.insert(ContextKind::InvalidArg, ContextValue::Strings(missing));
                    return Err(error);
                };
                let Some(gop_length) = self.gop_length else {
                    missing.push("--gop-length <GOP_LENGTH>".to_owned());
                    let mut error = clap::Error::new(ErrorKind::MissingRequiredArgument);
                    error.insert(ContextKind::InvalidArg, ContextValue::Strings(missing));
                    return Err(error);
                };
                Ok(L0bFormatConfig::MQ00 {
                    quantisation: self.quantisation,
                    dc_adjust: !self.no_dc_adjust,
                    wavelet_stages,
                    gop_length,
                })
            }
        }
    }
}

/// Exit using clap error formatting
pub fn exit_clap_error(error: clap::Error) -> ! {
    let mut command = Cli::command();
    command.build();

    if let Some(convert_command) = command.find_subcommand_mut("convert") {
        error.format(convert_command).exit();
    }
    error.exit()
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for LevelFilter {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Trace => Self::TRACE,
            LogLevel::Debug => Self::DEBUG,
            LogLevel::Info => Self::INFO,
            LogLevel::Warn => Self::WARN,
            LogLevel::Error => Self::ERROR,
        }
    }
}
