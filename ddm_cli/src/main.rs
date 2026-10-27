//! DDM CLI tools

use ddm_core::coding::wavelet::dwt_53_nd;
use ndarray::Array2;

mod backtrace;

use backtrace::install_error_reporting as install_eyre_hook;

fn main() -> color_eyre::eyre::Result<()> {
    install_eyre_hook()?;

    let mut data = Array2::from_shape_fn([4, 4], |(x, y)| {
        if (x + y) % 2 == 0 {
            i32::MAX
        } else {
            i32::MIN
        }
    });
    dwt_53_nd(&mut data.view_mut())?;

    println!("Hello, world!");
    Ok(())
}
