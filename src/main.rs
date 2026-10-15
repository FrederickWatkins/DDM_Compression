use anyhow::Result;
use clap::{Parser, ValueEnum};
use ddm_compression::app;
use tracing::Level;

#[derive(Debug, Parser)]
#[command(version, about = "DDM data browser and analyzer")]
struct Args {
    /// Set the logging verbosity.
    #[arg(short, long, value_enum, default_value = "warn")]
    log_level: LogLevel,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Self::TRACE,
            LogLevel::Debug => Self::DEBUG,
            LogLevel::Info => Self::INFO,
            LogLevel::Warn => Self::WARN,
            LogLevel::Error => Self::ERROR,
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_max_level(Level::from(args.log_level))
        .init();
    tracing::info!(log_level = ?args.log_level, "starting DDM Workbench");
    app::run()?;
    Ok(())
}
