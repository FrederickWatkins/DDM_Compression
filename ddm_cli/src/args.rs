use std::{convert::Infallible, iter::StepBy, ops::RangeInclusive, path::PathBuf, str::FromStr};

use clap::{
    Args, CommandFactory as _, Parser, Subcommand, ValueEnum,
    error::{ContextKind, ContextValue, ErrorKind},
    value_parser,
};
use color_eyre::eyre::{self, Context as _};
use ddm_app::{DDMFilters, L0bFormat, L0bFormatConfig};
use tracing::level_filters::LevelFilter;

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
    /// Benchmark DDM encoding methods with sweep variables
    Benchmark {
        /// .l0b.ddm file to read from
        #[arg(long, short)]
        input: PathBuf,
        /// .csv file to write to
        #[arg(long, short)]
        output: PathBuf,
        /// Sweep variables
        #[arg(long, short, value_name = "NAME=START-END:STEP")]
        sweep: Vec<Sweep>,
        /// Test configs. Repeat for each config.
        #[arg(long, short, required=true,
            allow_hyphen_values = true,
            value_parser = |s: &str| {
                BenchmarkL0bFormatArgs::try_parse_from(
                    std::iter::once(
                        "benchmark-config".to_owned()
                    ).chain(s.split_whitespace().map(str::to_owned)),
                )
            },
            long_help = "\
Test configs. Repeat for each config.

Each configuration is a quoted set of codec arguments:

  --config='--format mp00 --quantisation x'
  --config='--format mq00 --quantisation x --wavelet-stages 1 --gop-length 1'

Available format arguments:
  --format <FORMAT>          Format to encode as
  --quantisation <VALUE>     Quantisation (supports sweep variables)
  --no-dc-adjust             Disable DC adjustment
  --wavelet-stages <VALUE>   Number of wavelet stages (MQ00)
  --gop-length <VALUE>       GOP length (MQ00)",
)]
        config: Vec<BenchmarkL0bFormatArgs>,
    },
}

#[derive(Debug, Args)]
#[command(next_help_heading = "Format options")]
pub struct L0bFormatArgs {
    /// Format to encode as
    #[arg(long, short, value_enum, default_value = "mp00")]
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

#[derive(Debug, Clone)]
pub struct Sweep {
    pub name: String,
    pub range: StepBy<RangeInclusive<i64>>,
}

impl FromStr for Sweep {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('=');
        let Some(name) = parts.next().map(ToOwned::to_owned) else {
            return Err(eyre::eyre!("Empty sweep declaration"));
        };
        let Some(mut range_step_parts) = parts.next().map(|s| s.split(':')) else {
            return Err(eyre::eyre!(
                "No values present in sweep variable declaration"
            ));
        };
        let Some(range) = range_step_parts.next() else {
            return Err(eyre::eyre!(
                "No range present in sweep variable declaration"
            ));
        };
        let mut range_parts = range.split('-');
        let start = i64::from_str(match range_parts.next() {
            Some(s) => s,
            None => {
                return Err(eyre::eyre!("empty value in sweep variable range"))
                    .wrap_err_with(|| "error parsing start of range")
                    .map_err(|e| eyre::eyre!("{e:#}"));
            }
        })
        .wrap_err_with(|| "error parsing start of range")?;
        let end = i64::from_str(match range_parts.next() {
            Some(s) => s,
            None => {
                return Err(eyre::eyre!("empty value in sweep variable range"))
                    .wrap_err_with(|| "error parsing start of range")
                    .map_err(|e| eyre::eyre!("{e:#}"));
            }
        })
        .wrap_err_with(|| "error parsing end of range")?;
        let step = usize::from_str(range_step_parts.next().unwrap_or("1"))
            .wrap_err_with(|| "Error parsing step")?;
        let range = (start..=end).step_by(step);
        Ok(Self { name, range })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BenchmarkParameter<T> {
    Value(T),
    Variable(String),
}

impl<T: FromStr> FromStr for BenchmarkParameter<T> {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(T::from_str(s)
            .map(Self::Value)
            .unwrap_or(Self::Variable(s.to_owned())))
    }
}

#[derive(Debug, Clone, Parser)]
#[command(next_help_heading = "Format options")]
pub struct BenchmarkL0bFormatArgs {
    /// Format to encode as
    #[arg(long, short)]
    pub format: L0bFormat,
    /// Quantisation
    #[arg(long, short)]
    pub quantisation: BenchmarkParameter<u8>,
    #[arg(long)]
    pub no_dc_adjust: bool,
    #[arg(long)]
    pub wavelet_stages: Option<BenchmarkParameter<u8>>,
    #[arg(long)]
    pub gop_length: Option<BenchmarkParameter<u8>>,
}

pub enum BenchmarkL0bFormatConfig {
    /// MP00 format.
    MP00 {
        /// Pixel quantisation.
        quantisation: BenchmarkParameter<u8>,
    },
    /// MQ00 format.
    MQ00 {
        /// Wavelet coefficient quantisation.
        quantisation: BenchmarkParameter<u8>,
        /// Whether to apply DC adjustment.
        dc_adjust: bool,
        /// Number of wavelet transform stages.
        wavelet_stages: BenchmarkParameter<u8>,
        /// Length of a group of pictures.
        gop_length: BenchmarkParameter<u8>,
    },
}

impl TryInto<BenchmarkL0bFormatConfig> for BenchmarkL0bFormatArgs {
    type Error = clap::Error;

    fn try_into(self) -> Result<BenchmarkL0bFormatConfig, Self::Error> {
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
                    Ok(BenchmarkL0bFormatConfig::MP00 {
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
                Ok(BenchmarkL0bFormatConfig::MQ00 {
                    quantisation: self.quantisation,
                    dc_adjust: !self.no_dc_adjust,
                    wavelet_stages,
                    gop_length,
                })
            }
        }
    }
}
