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

mod app;
mod file_info;
mod image;
mod subsystem;

use generated::AppWindow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            "info,ddm_app=debug,ddm_workbench=debug",
        ))
        .compact()
        .init();

    app::run()?;
    Ok(())
}
