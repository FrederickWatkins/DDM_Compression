//! DDM command-line entry point.

use clap::Parser as _;
use color_eyre::eyre::{self, Result};
use ddm_cli::{Cli, Subcommands, benchmark, compare, convert, exit_clap_error, inspect};
use prodash::Root as _;

/// Parse command-line arguments and run the selected command.
///
/// # Errors
/// Returns errors raised by the selected command.
pub fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(args.log_level)
        .pretty()
        .compact()
        .with_target(true)
        .with_file(false)
        .with_thread_ids(true)
        .init();
    let root = prodash::tree::Root::new();
    let mut options = prodash::render::line::Options::default()
        .auto_configure(prodash::render::line::StreamKind::Stderr);

    options.terminal_dimensions.0 = options.terminal_dimensions.0.min(80);
    options.timestamp = false;
    options.throughput = true;
    options.frames_per_second = 8.0;
    options.initial_delay = Some(std::time::Duration::from_millis(250));
    options.level_filter = if args.quiet { Some(0..=0) } else { None };

    let renderer = prodash::render::line::render(std::io::stderr(), root.downgrade(), options);

    match args.command {
        Subcommands::Inspect { input } => {
            let mut progress = root.add_child("Inspect");
            let output = inspect(input, &mut progress)?;
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
            let format = format.try_into().map_err(exit_clap_error)?;
            let output = convert(input, output, &filters, &format, &mut progress)?;
            renderer.shutdown_and_wait();
            println!("{output}");
        }
        Subcommands::Compare { files } => {
            let mut progress = root.add_child("Compare");
            let output = compare(files, &mut progress)?;
            renderer.shutdown_and_wait();
            println!("{output}");
        }
        Subcommands::Benchmark {
            input,
            output,
            sweep,
            config,
        } => {
            let mut progress = root.add_child("Benchmark");
            let format = config
                .iter()
                .map(|c| Ok(c.clone().try_into()?))
                .collect::<eyre::Result<Vec<_>>>()?;
            dbg!(input.clone());
            dbg!(output.clone());
            dbg!(sweep.clone());
            dbg!(config.clone());
            benchmark(input, output, &sweep, &format, &mut progress)?;
        }
    }
    Ok(())
}
