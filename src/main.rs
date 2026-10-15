use ddm_compression::app;
use anyhow::Result;

fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    app::run()?;
    Ok(())
}