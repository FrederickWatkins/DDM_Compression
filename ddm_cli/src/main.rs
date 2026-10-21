//! DDM CLI
use std::path::PathBuf;

use clap::{
    Args, CommandFactory as _, Parser, Subcommand, ValueEnum,
    error::{ContextKind, ContextValue, ErrorKind},
    value_parser,
};
use ddm_app::{DDMFilters, L0bFormat, L0bFormatConfig};
use prodash::Root as _;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::convert::convert;

mod convert;
mod inspect;
mod table;

const WIDTH: usize = 20;
const HEIGHT: usize = 52;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Subcommands,
    #[arg(long, short, default_value = "warn")]
    log_level: LogLevel,
    #[arg(long, short)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Subcommands {
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
        #[command(flatten)]
        filters: DDMFilters,
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
struct L0bFormatArgs {
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
                let mut conflict = vec![];
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
                    let mut e = clap::Error::new(ErrorKind::ArgumentConflict);
                    e.insert(
                        ContextKind::InvalidArg,
                        ContextValue::String("--format mp00".to_owned()),
                    );
                    e.insert(ContextKind::PriorArg, ContextValue::Strings(conflict));
                    Err(e)
                }
            }
            L0bFormat::MQ00 => {
                if let (Some(wavelet_stages), Some(gop_length)) =
                    (self.wavelet_stages, self.gop_length)
                {
                    Ok(L0bFormatConfig::MQ00 {
                        quantisation: self.quantisation,
                        dc_adjust: !self.no_dc_adjust,
                        wavelet_stages,
                        gop_length,
                    })
                } else {
                    let mut missing = vec![];
                    if self.wavelet_stages.is_none() {
                        missing.push("--wavelet-stages <WAVELET_STAGES>".to_owned());
                    }
                    if self.gop_length.is_none() {
                        missing.push("--gop-length <GOP_LENGTH>".to_owned());
                    }
                    let mut e = clap::Error::new(ErrorKind::MissingRequiredArgument);
                    e.insert(ContextKind::InvalidArg, ContextValue::Strings(missing));
                    Err(e)
                }
            }
        }
    }
}

fn exit_clap_error(error: clap::Error) -> ! {
    let mut command = Cli::command();
    command.build();

    #[expect(
        clippy::expect_used,
        reason = "Can't recover from error during error handler"
    )]
    let convert_command = command
        .find_subcommand_mut("convert")
        .expect("convert subcommand exists");

    error.format(convert_command).exit()
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LogLevel {
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

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(args.log_level)
        .pretty()
        .compact()
        .with_target(true)
        .with_file(false)
        .init();
    let root = prodash::tree::Root::new();
    let mut options = prodash::render::line::Options::default()
        .auto_configure(prodash::render::line::StreamKind::Stderr);

    options.terminal_dimensions.0 = options.terminal_dimensions.0.min(80);
    options.timestamp = false;
    options.throughput = true;
    options.frames_per_second = 8.0;
    options.initial_delay = Some(std::time::Duration::from_millis(250));
    if args.quiet {
        options.level_filter = Some(0..=0);
    } else {
        options.level_filter = None;
    }

    let renderer = prodash::render::line::render(std::io::stderr(), root.downgrade(), options);

    match args.command {
        Subcommands::Inspect { input } => {
            let mut progress = root.add_child("Inspect");
            let output = inspect::inspect(input, &mut progress)?;
            renderer.shutdown_and_wait();
            println!("{output}");
        }
        Subcommands::Convert {
            input,
            output,
            filters,
            format,
        } => {
            let mut progress = root.add_child("Convert");
            let format = format.try_into().map_err(|e| exit_clap_error(e))?;
            let output = convert(input, output, &filters, &format, &mut progress)?;
            renderer.shutdown_and_wait();
            println!("{output}");
        }

        Subcommands::Compare { .. } => todo!(),
    }
    Ok(())
}
