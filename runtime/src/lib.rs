//! Plugin runtime: loads plugins packaged as WebAssembly components and runs them in a sandbox,
//! under limits set by the host.

mod error;
mod limits;

pub use error::{ErrorKind, RuntimeError};
pub use limits::Limits;
