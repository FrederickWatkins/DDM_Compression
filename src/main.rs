use anyhow::Result;
use ddm_compression::app;

fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    app::run()?;
    Ok(())
}
