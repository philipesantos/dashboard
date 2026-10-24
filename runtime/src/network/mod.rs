//! The network module: the only code that opens connections and the only user of `ureq`
//! (`specs/003-plugin-connectors/research.md`, R5–R10, R20).

mod address;
mod resolver;

#[cfg(feature = "test-network")]
pub use resolver::{Route, TestNetwork};

/// How the runtime reaches connector destinations: production DNS and Mozilla's roots, or, in
/// the runtime's own tests, the test seam.
#[derive(Debug, Clone, Default)]
pub(crate) struct Network {
    #[cfg(feature = "test-network")]
    seam: Option<std::sync::Arc<TestNetwork>>,
}

impl Network {
    pub(crate) fn production() -> Network {
        Network::default()
    }

    #[cfg(feature = "test-network")]
    pub(crate) fn with_seam(seam: TestNetwork) -> Network {
        Network {
            seam: Some(std::sync::Arc::new(seam)),
        }
    }
}
