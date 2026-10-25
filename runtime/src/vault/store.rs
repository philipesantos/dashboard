//! The keychain store of each system, and the only use of `keyring-core` and the store crates
//! (`specs/005-plugin-credentials/research.md`, R5, R19). Every `keyring-core` error is mapped
//! here to a [`VaultError`] that holds no credential: `BadEncoding` and `BadDataFormat` carry the
//! raw bytes of the entry, and `keyring_core::Error` derives `Debug`, so no value of the crate is
//! ever formatted.

use std::sync::Arc;

use keyring_core::{CredentialStore, Entry, Error};

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
    /// A fresh in-memory store, which keeps entries for as long as it lives.
    #[cfg(any(test, feature = "test-network"))]
    pub(crate) fn mock() -> Backend {
        Backend::Mock(keyring_core::mock::Store::new().expect("the mock store is created"))
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
    store: Option<Arc<CredentialStore>>,
}

impl Opened {
    pub(crate) fn new(backend: Backend) -> Opened {
        Opened {
            backend,
            store: None,
        }
    }

    fn store(&mut self) -> Result<Arc<CredentialStore>, VaultError> {
        if let Some(store) = &self.store {
            return Ok(Arc::clone(store));
        }
        let store: Arc<CredentialStore> = match &self.backend {
            Backend::System => system_store()?,
            #[cfg(any(test, feature = "test-network"))]
            Backend::Mock(store) => Arc::clone(store) as Arc<CredentialStore>,
        };
        self.store = Some(Arc::clone(&store));
        Ok(store)
    }

    fn entry(&mut self, service: &str, user: &str) -> Result<Entry, VaultError> {
        self.store()?.build(service, user, None).map_err(map)
    }

    pub(crate) fn get(&mut self, service: &str, user: &str) -> Result<Vec<u8>, VaultError> {
        self.entry(service, user)?.get_secret().map_err(map)
    }

    pub(crate) fn set(&mut self, service: &str, user: &str, secret: &[u8]) -> Result<(), VaultError> {
        self.entry(service, user)?.set_secret(secret).map_err(map)
    }

    /// Deletes the entry; `false` when there was none.
    pub(crate) fn delete(&mut self, service: &str, user: &str) -> Result<bool, VaultError> {
        match self.entry(service, user)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(Error::NoEntry) => Ok(false),
            Err(error) => Err(map(error)),
        }
    }

    /// Makes the next operation on `user` answer `failure`, once.
    #[cfg(any(test, feature = "test-network"))]
    pub(crate) fn fail_next(&mut self, service: &str, user: &str, failure: Failure) {
        let entry = self
            .entry(service, user)
            .expect("the mock store builds every entry");
        let error = match failure {
            Failure::NoStorageAccess => Error::NoStorageAccess(platform("locked")),
            Failure::PlatformFailure => Error::PlatformFailure(platform("failed")),
            Failure::BadEncoding(bytes) => Error::BadEncoding(bytes),
            Failure::BadDataFormat(bytes) => Error::BadDataFormat(bytes, platform("bad format")),
        };
        entry
            .as_any()
            .downcast_ref::<keyring_core::mock::Cred>()
            .expect("an entry of the mock store is a mock credential")
            .set_error(error);
    }
}

#[cfg(any(test, feature = "test-network"))]
fn platform(text: &str) -> Box<dyn std::error::Error + Send + Sync> {
    text.to_owned().into()
}

/// The keychain of this system (research R5).
fn system_store() -> Result<Arc<CredentialStore>, VaultError> {
    #[cfg(target_os = "linux")]
    let store = dbus_secret_service_keyring_store::Store::new()
        .map(|store| store as Arc<CredentialStore>);
    #[cfg(windows)]
    let store = windows_native_keyring_store::Store::new()
        .map(|store| store as Arc<CredentialStore>);
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new()
        .map(|store| store as Arc<CredentialStore>);
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    let store: Result<Arc<CredentialStore>, Error> = Err(Error::NotSupportedByStore(
        "no keychain store for this system".to_owned(),
    ));
    store.map_err(map)
}

/// The error of `keyring-core`, as text that holds no credential. The platform's own message is
/// kept for the failures that come from the keychain itself, cut to a line; the variants that
/// carry an entry's bytes keep none of them.
fn map(error: Error) -> VaultError {
    let platform = |error: &(dyn std::error::Error + Send + Sync)| {
        let text = error.to_string();
        let line = text.lines().next().unwrap_or_default();
        line.chars().take(200).collect::<String>()
    };
    match error {
        Error::NoEntry => VaultError::NoEntry,
        Error::NoStorageAccess(error) => VaultError::Unavailable(format!(
            "it refused access, as when it is locked ({})",
            platform(error.as_ref())
        )),
        Error::PlatformFailure(error) => {
            VaultError::Unavailable(format!("it failed ({})", platform(error.as_ref())))
        }
        Error::BadEncoding(_) | Error::BadDataFormat(..) => VaultError::Unreadable,
        Error::TooLong(attribute, limit) => VaultError::Unavailable(format!(
            "it refused an entry whose {attribute} is longer than {limit}"
        )),
        _ => VaultError::Unavailable("it answered with an error".to_owned()),
    }
}
