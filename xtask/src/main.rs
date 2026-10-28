//! Project automation

use std::path::PathBuf;
use std::{env, };
use std::process::{self, Command, };

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask crate must be a subdirectory of project root").to_path_buf()
}

fn main() -> ! {
    // Collect arguments. 
    // Note: When running via `cargo run --package xtask -- <args>`, 
    // env::args() drops the cargo wrapper and starts with the binary or first user arg.
    let mut args = env::args().skip(1);

    let Some(command) = args.next() else {
        eprintln!("Error: No command provided.");
        process::exit(1);
    };

    // Collect all remaining arguments into a vector
    let remaining_args: Vec<String> = args.collect();

    match command.as_str() {
        "visualise" | "py" => {
            run_python_script(&remaining_args);
        }
        unknown => {
            eprintln!("Unknown xtask command: '{unknown}'");
            process::exit(1);
        }
    }
}

fn run_python_script(args: &[String]) -> ! {
    let mut python_script = workspace_root();
    python_script.push("visualise_py/main.py");
    let status = Command::new("python3")
        .arg(python_script)
        .args(args)
        .status().unwrap_or_else(|e| {eprintln!("{e}"); process::exit(1)});

    // Exit with the same code returned by the Python script
    if status.success() {
        process::exit(0);
    }
    process::exit(status.code().unwrap_or(1));
}
