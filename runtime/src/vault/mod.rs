#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
//! The secrets vault: credentials kept in the system's keychain through keyring-rs, and nowhere
//! else (`specs/005-plugin-credentials/research.md`, R5; constitution Principle IV). `store.rs`
//! is the only code that touches `keyring-core` or a store crate, as `clippy.toml` enforces.
//!
//! Every operation runs on the vault's own thread, one at a time and in the order given: the
//! Windows store does not order operations on one entry made from several threads. A caller
//! waits at most a bound and then gets `keychain-unavailable`; the operation still runs before
//! any later one, so nothing overtakes it. No error the vault returns holds a credential.

mod store;

use std::fmt;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub(crate) use store::Backend;
#[cfg(any(test, feature = "test-network"))]
pub(crate) use store::Failure;

/// The keychain service of the development vault (constitution v2.4.0, Principle VII). Installed
/// plugins will use their own from Phase 3.
pub(crate) const DEVELOPMENT_SERVICE: &str = "dev.philsantos.dashboard.development";

/// A credential held in the host's memory: wiped when dropped, never printed.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Secret(Zeroizing<String>);

impl Secret {
    pub(crate) fn new(value: String) -> Secret {
        Secret(Zeroizing::new(value))
    }

    /// The credential itself, for the one place that sends it: the network module.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[credential]")
    }
}

/// What the vault keeps for one account: the document of `data-model.md` (Vault entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Stored {
    Account {
        access: Secret,
        refresh: Option<Secret>,
    },
    Key {
        key: Secret,
    },
}

/// The JSON form of [`Stored`], written from borrowed strings into a buffer wiped on drop.
#[derive(Serialize)]
struct DocumentOut<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    access: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<&'a str>,
}

/// The JSON form of [`Stored`] as read: each string moves into a [`Secret`] at once, which wipes
/// it when dropped.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentIn {
    #[serde(default)]
    access: Option<String>,
    #[serde(default)]
    refresh: Option<String>,
    #[serde(default)]
    key: Option<String>,
}

impl Stored {
    fn to_bytes(&self) -> Zeroizing<Vec<u8>> {
        let document = match self {
            Stored::Account { access, refresh } => DocumentOut {
                access: Some(access.expose()),
                refresh: refresh.as_ref().map(Secret::expose),
                key: None,
            },
            Stored::Key { key } => DocumentOut {
                access: None,
                refresh: None,
                key: Some(key.expose()),
            },
        };
        Zeroizing::new(serde_json::to_vec(&document).expect("a document of strings serializes"))
    }

    /// The stored document, or `None` when it is not one: never the parser's error, which
    /// quotes the value it could not read.
    fn from_bytes(bytes: &[u8]) -> Option<Stored> {
        let document: DocumentIn = serde_json::from_slice(bytes).ok()?;
        let DocumentIn {
            access,
            refresh,
            key,
        } = document;
        let (access, refresh, key) = (
            access.map(Secret::new),
            refresh.map(Secret::new),
            key.map(Secret::new),
        );
        match (access, refresh, key) {
            (Some(access), refresh, None) => Some(Stored::Account { access, refresh }),
            (None, None, Some(key)) => Some(Stored::Key { key }),
            _ => None,
        }
    }
}

/// Why a vault operation did not happen. None of these hold a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VaultError {
    /// The keychain is locked, missing, refused access or did not answer in time.
    Unavailable(String),
    /// No entry for this account.
    NoEntry,
    /// The document would not fit in what every system's keychain keeps.
    TooLarge { bytes: usize, limit: usize },
    /// The entry exists but holds something other than a vault document.
    Unreadable,
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VaultError::Unavailable(why) => write!(f, "the keychain is unavailable: {why}"),
            VaultError::NoEntry => f.write_str("the keychain has no entry for this account"),
            VaultError::TooLarge { bytes, limit } => write!(
                f,
                "the credential takes {bytes} bytes, over the {limit} bytes every system's keychain keeps"
            ),
            VaultError::Unreadable => {
                f.write_str("the keychain's entry for this account is not one the host wrote")
            }
        }
    }
}

/// The keychain user name of an account: the vault's id and the SHA-256 of the connector's id
/// and origin (`data-model.md`, Vault entry).
pub(crate) fn entry_user(vault_id: &str, connector: &str, origin: &str) -> String {
    let digest = Sha256::digest(format!("{connector}\n{origin}").as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{vault_id}:{hex}")
}

type Job = Box<dyn FnOnce(&mut store::Opened) + Send>;

/// The vault of one runtime: a thread that owns the store and runs every operation in order.
pub(crate) struct Vault {
    jobs: mpsc::Sender<Job>,
    service: &'static str,
    max_secret_bytes: usize,
}

impl fmt::Debug for Vault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vault")
            .field("service", &self.service)
            .finish()
    }
}

/// An operation sent to the vault's thread, whose answer a caller waits for within a bound.
pub(crate) struct Pending<T> {
    answer: mpsc::Receiver<Result<T, VaultError>>,
}

impl<T> Pending<T> {
    /// The answer, or `Unavailable` when the keychain did not answer within `bound`. The
    /// operation still runs; nothing sent later overtakes it.
    pub(crate) fn within(self, bound: Duration) -> Result<T, VaultError> {
        match self.answer.recv_timeout(bound) {
            Ok(answer) => answer,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(VaultError::Unavailable(format!(
                "it did not answer within {} s",
                bound.as_secs()
            ))),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(VaultError::Unavailable("its thread stopped".to_owned()))
            }
        }
    }
}

impl Vault {
    /// The vault over `backend`, with its thread started; the store itself opens on first use,
    /// so a command that never touches the keychain never connects to it.
    pub(crate) fn new(backend: Backend, max_secret_bytes: usize) -> Vault {
        Vault {
            jobs: spawn(backend),
            service: DEVELOPMENT_SERVICE,
            max_secret_bytes,
        }
    }

    pub(crate) fn read(&self, user: &str) -> Pending<Stored> {
        let (service, user) = (self.service, user.to_owned());
        self.send(move |opened| {
            let bytes = Zeroizing::new(opened.get(service, &user)?);
            Stored::from_bytes(&bytes).ok_or(VaultError::Unreadable)
        })
    }

    pub(crate) fn write(&self, user: &str, stored: &Stored) -> Pending<()> {
        let bytes = stored.to_bytes();
        if bytes.len() > self.max_secret_bytes {
            let (sender, answer) = mpsc::sync_channel(1);
            let _ = sender.send(Err(VaultError::TooLarge {
                bytes: bytes.len(),
                limit: self.max_secret_bytes,
            }));
            return Pending { answer };
        }
        let (service, user) = (self.service, user.to_owned());
        self.send(move |opened| opened.set(service, &user, &bytes))
    }

    /// Deletes the entry; `false` when there was none.
    pub(crate) fn delete(&self, user: &str) -> Pending<bool> {
        let (service, user) = (self.service, user.to_owned());
        self.send(move |opened| opened.delete(service, &user))
    }

    /// Makes the vault's thread wait `delay` before its next operation, for the tests.
    #[cfg(any(test, feature = "test-network"))]
    pub(crate) fn delay_next(&self, delay: Duration) {
        drop(self.send(move |_| {
            thread::sleep(delay);
            Ok(())
        }));
    }

    /// Makes the next operation on `user` fail with `failure`, for the tests.
    #[cfg(any(test, feature = "test-network"))]
    pub(crate) fn fail_next(&self, user: &str, failure: store::Failure) {
        let (service, user) = (self.service, user.to_owned());
        let done = self.send(move |opened| {
            opened.fail_next(service, &user, failure);
            Ok(())
        });
        done.within(Duration::from_secs(5))
            .expect("the mock store takes a failure");
    }

    /// Writes `bytes` as `user`'s entry as they are, for the tests of entries the host did not
    /// write.
    #[cfg(any(test, feature = "test-network"))]
    pub(crate) fn write_raw(&self, user: &str, bytes: Vec<u8>) {
        let (service, user) = (self.service, user.to_owned());
        self.send(move |opened| opened.set(service, &user, &bytes))
            .within(Duration::from_secs(5))
            .expect("the mock store keeps the entry");
    }

    /// Runs `job` on the vault's thread, after every operation sent before it.
    fn send<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut store::Opened) -> Result<T, VaultError> + Send + 'static,
    ) -> Pending<T> {
        let (sender, answer) = mpsc::sync_channel(1);
        let job: Job = Box::new(move |opened| {
            // The caller may have stopped waiting; the answer is then dropped
            let _ = sender.send(job(opened));
        });
        if self.jobs.send(job).is_err() {
            let (sender, answer) = mpsc::sync_channel(1);
            let _ = sender.send(Err(VaultError::Unavailable(
                "its thread stopped".to_owned(),
            )));
            return Pending { answer };
        }
        Pending { answer }
    }
}

/// Starts the thread that owns the store.
fn spawn(backend: Backend) -> mpsc::Sender<Job> {
    let (jobs, queue) = mpsc::channel::<Job>();
    thread::Builder::new()
        .name("vault".to_owned())
        .spawn(move || {
            let mut opened = store::Opened::new(backend);
            for job in queue {
                job(&mut opened);
            }
        })
        .expect("the vault's thread starts");
    jobs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    const WAIT: Duration = Duration::from_secs(5);
    const TOKEN: &str = "gho_7f3a9c1e5b2d4f6a8c0e2b4d6f8a0c2e4b6d8f0a";

    fn vault() -> Vault {
        Vault::new(Backend::mock(), 2560)
    }

    fn account(access: &str, refresh: Option<&str>) -> Stored {
        Stored::Account {
            access: Secret::new(access.to_owned()),
            refresh: refresh.map(|refresh| Secret::new(refresh.to_owned())),
        }
    }

    #[test]
    fn entries_are_named_by_the_vault_id_connector_and_origin() {
        assert_eq!(DEVELOPMENT_SERVICE, "dev.philsantos.dashboard.development");
        let user = entry_user("5f0c", "local/notes", "/home/ada/notes.json");
        let digest = Sha256::digest(b"local/notes\n/home/ada/notes.json");
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(user, format!("5f0c:{hex}"));
        assert_ne!(
            user,
            entry_user("5f0c", "local/notes", "/home/ada/other.json")
        );
        assert_ne!(
            user,
            entry_user("9a1b", "local/notes", "/home/ada/notes.json")
        );
    }

    #[test]
    fn accounts_and_keys_round_trip_and_an_absent_entry_is_no_entry() {
        let vault = vault();
        assert_eq!(vault.read("a").within(WAIT), Err(VaultError::NoEntry));
        let stored = account(TOKEN, Some("ghr_1234"));
        vault.write("a", &stored).within(WAIT).unwrap();
        assert_eq!(vault.read("a").within(WAIT).unwrap(), stored);
        let key = Stored::Key {
            key: Secret::new("k-1".to_owned()),
        };
        vault.write("b", &key).within(WAIT).unwrap();
        assert_eq!(vault.read("b").within(WAIT).unwrap(), key);
        // An account without a refresh token is kept as one
        vault
            .write("a", &account(TOKEN, None))
            .within(WAIT)
            .unwrap();
        assert_eq!(vault.read("a").within(WAIT).unwrap(), account(TOKEN, None));
        assert_eq!(vault.delete("a").within(WAIT), Ok(true));
        assert_eq!(vault.delete("a").within(WAIT), Ok(false));
        assert_eq!(vault.read("a").within(WAIT), Err(VaultError::NoEntry));
    }

    #[test]
    fn a_document_over_the_limit_never_reaches_the_store() {
        let vault = vault();
        // {"key":"…"} is 10 bytes around the key
        let fits = Stored::Key {
            key: Secret::new("x".repeat(2560 - 10)),
        };
        vault.write("fits", &fits).within(WAIT).unwrap();
        let over = Stored::Key {
            key: Secret::new("x".repeat(2560 - 9)),
        };
        assert_eq!(
            vault.write("over", &over).within(WAIT),
            Err(VaultError::TooLarge {
                bytes: 2561,
                limit: 2560
            })
        );
        assert_eq!(vault.read("over").within(WAIT), Err(VaultError::NoEntry));
    }

    #[test]
    fn operations_run_in_order_and_one_given_up_on_still_runs_first() {
        // vault_order: a slow write followed by a read returns what was written
        let vault = vault();
        vault.delay_next(Duration::from_millis(300));
        let slow = vault.write("a", &account(TOKEN, None));
        let started = Instant::now();
        assert!(matches!(
            slow.within(Duration::from_millis(50)),
            Err(VaultError::Unavailable(_))
        ));
        assert!(started.elapsed() < Duration::from_millis(250));
        assert_eq!(vault.read("a").within(WAIT).unwrap(), account(TOKEN, None));
    }

    #[test]
    fn a_locked_or_failing_keychain_is_unavailable() {
        let vault = vault();
        vault
            .write("a", &account(TOKEN, None))
            .within(WAIT)
            .unwrap();
        vault.fail_next("a", store::Failure::NoStorageAccess);
        assert!(matches!(
            vault.read("a").within(WAIT),
            Err(VaultError::Unavailable(_))
        ));
        vault.fail_next("a", store::Failure::Platform);
        assert!(matches!(
            vault.read("a").within(WAIT),
            Err(VaultError::Unavailable(_))
        ));
        // Once the error was answered, the entry is there again
        assert_eq!(vault.read("a").within(WAIT).unwrap(), account(TOKEN, None));
    }

    /// secret_free_errors: errors that carry the secret's bytes, and a stored document that does
    /// not parse, give errors that hold none of them.
    #[test]
    fn errors_never_hold_the_secret() {
        let vault = vault();
        vault
            .write("a", &account(TOKEN, None))
            .within(WAIT)
            .unwrap();
        for failure in [
            store::Failure::BadEncoding(TOKEN.as_bytes().to_vec()),
            store::Failure::BadDataFormat(TOKEN.as_bytes().to_vec()),
        ] {
            vault.fail_next("a", failure);
            let error = vault.read("a").within(WAIT).unwrap_err();
            for text in [error.to_string(), format!("{error:?}")] {
                assert!(!text.contains(TOKEN), "{text}");
                assert!(!text.contains("7f3a9c1e"), "{text}");
            }
        }
        vault.write_raw(
            "b",
            format!("{{\"access\": \"{TOKEN}\", \"extra\": 1}}").into_bytes(),
        );
        let error = vault.read("b").within(WAIT).unwrap_err();
        assert_eq!(error, VaultError::Unreadable);
        assert!(!format!("{error} {error:?}").contains(TOKEN));
    }

    #[test]
    fn a_secret_prints_as_a_placeholder() {
        let secret = Secret::new(TOKEN.to_owned());
        assert_eq!(format!("{secret:?}"), "[credential]");
        assert_eq!(
            format!("{:?}", account(TOKEN, Some("ghr_9"))),
            "Account { access: [credential], refresh: Some([credential]) }"
        );
        assert_eq!(secret.expose(), TOKEN);
    }
}
