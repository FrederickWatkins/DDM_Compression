//! DDM CLI
use std::path::PathBuf;

use clap::{
    Args, CommandFactory as _, Parser, Subcommand,
    error::{ContextKind, ContextValue, ErrorKind},
    value_parser,
};
use ddm_app::{DDMFilters, L0bFormat, L0bFormatConfig, convert, inspect};
use tracing::{info, level_filters::LevelFilter};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Subcommands,
}

#[derive(Subcommand)]
enum Subcommands {
    /// View DDM metadata
    Inspect {
        /// Input .l0b.ddm file
        #[arg(long, short)]
        input: PathBuf,
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

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .init();
    info!("Tracing");
    let args = Cli::parse();

    match args.command {
        Subcommands::Inspect { input } => {
            println!("{}", inspect::<20, 52>(input)?);
        }
        Subcommands::Convert {
            input,
            output,
            filters,
            format,
        } => convert::<20, 52>(
            input,
            output,
            &filters,
            format
                .try_into()
                .unwrap_or_else(|e: clap::Error| exit_clap_error(e)),
        )?,
    }
    Ok(())
}
