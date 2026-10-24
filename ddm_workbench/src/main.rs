//! DDM Workbench desktop application.

mod generated {
    #![allow(
        clippy::all,
        clippy::pedantic,
        clippy::restriction,
        reason = "Slint-generated Rust code is maintained by the Slint compiler"
    )]
    #![expect(
        unused_qualifications,
        reason = "Slint-generated Rust code uses fully qualified paths"
    )]

    slint::include_modules!();
}

mod subsystem;

use std::fmt::Display;

use generated::{MainWindow, FileType, RunAction, TaskInfo};
use slint::ComponentHandle as _;
use tracing::debug;

impl Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::L0bArchive => "L0b archive",
            Self::Image => "image",
            Self::BenchmarkResults => "benchmark results",
        })
    }
}

impl Display for RunAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Benchmark => "benchmark",
        })
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            "info,ddm_app=debug,ddm_workbench=debug",
        ))
        .compact()
        .init();
    let window = MainWindow::new()?;
    window.on_open_file(|file_type| debug!("Open file {file_type} requested"));
    window.on_export_file(|file_type| debug!("Export file {file_type} requested"));
    window.on_run_action(|run_action| debug!("Run action {run_action} requested"));
    window.set_tasks((&vec![TaskInfo{ id: 1, name: "Load L0b".into(), progress: 60.0 }] as &[TaskInfo]).into());
    window.run()?;
    Ok(())
}
