//! DDM compression core library
#![cfg_attr(
    test,
    allow(
        clippy::panic_in_result_fn,
        reason = "Test functions return results but use assertions"
    )
)]
pub mod coding;
pub mod l0b;
