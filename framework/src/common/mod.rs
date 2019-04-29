pub mod errors;
pub use self::errors::Result;
//pub use self::errors::ResultExt;
pub use self::errors::ErrorKind;

/// Null metadata associated with packets initially.
pub struct EmptyMetadata;

/*
pub fn print_error(e: &Error) {
    error!("Error: {}", e);
    for e in e.iter().skip(1) {
        error!("Cause: {}", e);
    }
    if let Some(backtrace) = e.backtrace() {
        error!("Backtrace: {:?}", backtrace);
    }
}
*/
