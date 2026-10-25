use std::any::Any;

use eyre::eyre;

/// Downcast a boxed thread join error into an eyre error
pub fn downcast_join_err(err: &Box<dyn Any + Send>) -> eyre::Report {
    if let Some(s) = err.downcast_ref::<&str>() {
        eyre!(s.to_owned())
    } else if let Some(s) = err.downcast_ref::<String>() {
        eyre!(s.to_owned())
    } else {
        eyre!("Thread pancked with unknown payload")
    }
}

/// Downcast a boxed thread join result into an eyre result
///
/// # Errors
/// If the result is an error...
pub fn map_join_err<T>(res: Result<T, Box<dyn Any + Send>>) -> eyre::Result<T> {
    res.map_err(|e| downcast_join_err(&e))
}
