//! The keychain store of each system, and the only use of `keyring-core` and the store crates
//! (`specs/005-plugin-credentials/research.md`, R5, R19).

use std::sync::Arc;

use super::VaultError;

/// Which store the vault opens: the system's keychain, or, in the runtime's own tests, an
/// in-memory one.
#[derive(Debug, Clone)]
pub(crate) enum Backend {
    System,
    #[cfg(any(test, feature = "test-network"))]
    Mock(Arc<keyring_core::mock::Store>),
}

impl Backend {
    #[cfg(any(test, feature = "test-network"))]
    pub(crate) fn mock() -> Backend {
        todo!("T013")
    }
}

/// A failure the in-memory store answers with once, for the tests.
#[cfg(any(test, feature = "test-network"))]
#[derive(Debug, Clone)]
pub(crate) enum Failure {
    NoStorageAccess,
    PlatformFailure,
    BadEncoding(Vec<u8>),
    BadDataFormat(Vec<u8>),
}

/// The store, opened on first use by the vault's thread.
pub(crate) struct Opened {
    backend: Backend,
}

impl Opened {
    pub(crate) fn new(backend: Backend) -> Opened {
        Opened { backend }
    }
}
