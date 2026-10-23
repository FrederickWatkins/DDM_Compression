//! Compile the Slint UI into Rust bindings.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint_build::compile("ui/main.slint")?;
    Ok(())
}
