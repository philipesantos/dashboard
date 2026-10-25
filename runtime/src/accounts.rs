//! The accounts record: which account is connected to each connector, never a credential
//! (`specs/005-plugin-credentials/research.md`, R6, R11, R12; `data-model.md`, Accounts record).
//!
//! `<data dir>/accounts.json` is host data, like the grant record: read on every use, changed
//! under its lock file with an atomic rename. The credentials themselves are in the vault, under
//! entries named by the record's vault id; changing the record and the vault together happens
//! under the record's lock, so no other process sees one without the other.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::connector::Access;
use crate::error::{ErrorKind, RuntimeError};
use crate::records;

/// The version of the accounts record this host writes.
pub const RECORD_VERSION: u64 = 1;

/// The record's file name in the data directory.
pub const FILE_NAME: &str = "accounts.json";

/// One connected account (`data-model.md`, Accounts record).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub connector: String,
    /// `catalog`, or the canonical path of a local connector's file.
    pub origin: String,
    pub kind: AccountKind,
    /// `authorization-code`; absent for a key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    /// The client registration the account was connected with, used again to refresh it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<Client>,
    /// New for a new account, kept when the same account connects again (research R12).
    pub account_id: String,
    /// The account's name, at most 100 characters; `key` for a key, `account` when the
    /// connector cannot tell.
    pub name: String,
    /// The account's stable id at the service, from the identity operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    /// The connector's access when the account was connected.
    #[serde(default)]
    pub access_asked: Vec<Access>,
    /// The scopes the token response named, as given.
    #[serde(default)]
    pub access_reported: Vec<String>,
    pub connected_at_ms: u64,
    pub refreshed_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<u64>,
    #[serde(default)]
    pub needs_connecting: Option<NeedsConnecting>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Account,
    Key,
}

/// A client registration: who the host is at the service. Never a secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Client {
    pub client_id: String,
    /// `option` (the development tool's `--client`) or `built-in`.
    pub from: String,
}

/// Why an account must be connected again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeedsConnecting {
    pub reason: Reason,
    pub at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// The service refused the credential, and the identity operation too.
    Refused,
    /// The token could not be refreshed, or the refreshed one could not be kept.
    RefreshFailed,
    /// The keychain has no entry for an account the record says is connected.
    MissingFromKeychain,
    /// The connector now asks for more access than the account was connected with.
    LessAccess,
}

/// An account about to be connected, before the record gives it an account id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAccount {
    pub connector: String,
    pub origin: String,
    pub kind: AccountKind,
    pub flow: Option<String>,
    pub client: Option<Client>,
    pub name: String,
    pub service_id: Option<String>,
    pub access_asked: Vec<Access>,
    pub access_reported: Vec<String>,
    pub now_ms: u64,
    pub expires_at_ms: Option<u64>,
}

/// What connecting did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connected {
    New(Account),
    /// The same account connected again: its account id kept.
    Again(Account),
}

/// The accounts record of one data directory.
#[derive(Debug, Clone)]
pub struct AccountStore {
    path: PathBuf,
}

impl AccountStore {
    /// The record in `data_dir`; nothing is created until something is connected.
    pub fn open(data_dir: &Path) -> AccountStore {
        AccountStore {
            path: data_dir.join(FILE_NAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The vault id of this record, created with the record the first time it is asked for.
    pub fn vault_id(&self) -> Result<String, RuntimeError> {
        if let Some(record) = self.read()? {
            return Ok(record.vault);
        }
        self.change(|record| Ok((record.vault.clone(), true)))
    }

    /// The account connected to `connector` from `origin`, if any.
    pub fn get(&self, connector: &str, origin: &str) -> Result<Option<Account>, RuntimeError> {
        Ok(self.read()?.and_then(|record| {
            record
                .accounts
                .into_iter()
                .find(|account| account.connector == connector && account.origin == origin)
        }))
    }

    /// Every account, sorted by connector id and origin.
    pub fn list(&self) -> Result<Vec<Account>, RuntimeError> {
        let mut accounts = self.read()?.map(|record| record.accounts).unwrap_or_default();
        accounts.sort_by(|a, b| (&a.connector, &a.origin).cmp(&(&b.connector, &b.origin)));
        Ok(accounts)
    }

    /// Connects `new` under the record's lock: the same account (same `service_id`) keeps its
    /// account id; another account, one the connector cannot identify, or a key while one is
    /// stored is `another-account-connected`, naming the current one, and nothing changes.
    /// `write_vault` runs, still under the lock, after the comparison and before the record is
    /// written; when it fails, the record is left as it was.
    pub fn connect(
        &self,
        new: NewAccount,
        write_vault: impl FnOnce(&Account) -> Result<(), RuntimeError>,
    ) -> Result<Connected, RuntimeError> {
        self.change(|record| {
            let position = record
                .accounts
                .iter()
                .position(|account| account.connector == new.connector && account.origin == new.origin);
            let current = position.map(|index| record.accounts[index].clone());
            let same = |current: &Account| {
                current.kind == AccountKind::Account
                    && new.kind == AccountKind::Account
                    && current.service_id.is_some()
                    && current.service_id == new.service_id
            };
            let account_id = match &current {
                Some(current) if same(current) => current.account_id.clone(),
                Some(current) => {
                    return Err(RuntimeError::new(
                        ErrorKind::AnotherAccountConnected,
                        format!(
                            "{} already has {} connected; disconnect it first (runtime accounts disconnect {})",
                            new.connector,
                            describe(current),
                            new.connector
                        ),
                    ));
                }
                None => random_hex()?,
            };
            let account = Account {
                connector: new.connector,
                origin: new.origin,
                kind: new.kind,
                flow: new.flow,
                client: new.client,
                account_id,
                name: new.name.chars().take(MAX_NAME_CHARS).collect(),
                service_id: new.service_id,
                access_asked: new.access_asked,
                access_reported: new.access_reported,
                connected_at_ms: new.now_ms,
                refreshed_at_ms: new.now_ms,
                expires_at_ms: new.expires_at_ms,
                needs_connecting: None,
            };
            write_vault(&account)?;
            let connected = match position {
                Some(index) => {
                    record.accounts[index] = account.clone();
                    Connected::Again(account)
                }
                None => {
                    record.accounts.push(account.clone());
                    Connected::New(account)
                }
            };
            Ok((connected, true))
        })
    }

    /// Disconnects the account under the record's lock: `delete_vault` runs first, then the
    /// account leaves the record, so no call between the two can mark it as missing. `None` when
    /// nothing is connected.
    pub fn disconnect(
        &self,
        connector: &str,
        origin: &str,
        delete_vault: impl FnOnce(&Account) -> Result<(), RuntimeError>,
    ) -> Result<Option<Account>, RuntimeError> {
        if self.read()?.is_none() {
            return Ok(None);
        }
        self.change(|record| {
            let Some(index) = record
                .accounts
                .iter()
                .position(|account| account.connector == connector && account.origin == origin)
            else {
                return Ok((None, false));
            };
            delete_vault(&record.accounts[index])?;
            Ok((Some(record.accounts.remove(index)), true))
        })
    }

    /// Marks the account as needing connecting again, only if it is still the one `account_id`
    /// names.
    pub fn mark_needs_connecting(
        &self,
        connector: &str,
        origin: &str,
        account_id: &str,
        reason: Reason,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.update(connector, origin, account_id, |account| {
            account.needs_connecting = Some(NeedsConnecting { reason, at_ms: now_ms });
        })
        .map(|_| ())
    }

    /// Changes the account under the record's lock, only if it is still the one `account_id`
    /// names; `false` when it is not there any more.
    pub(crate) fn update(
        &self,
        connector: &str,
        origin: &str,
        account_id: &str,
        change: impl FnOnce(&mut Account),
    ) -> Result<bool, RuntimeError> {
        if self.read()?.is_none() {
            return Ok(false);
        }
        self.change(|record| {
            let Some(account) = record.accounts.iter_mut().find(|account| {
                account.connector == connector
                    && account.origin == origin
                    && account.account_id == account_id
            }) else {
                return Ok((false, false));
            };
            change(account);
            Ok((true, true))
        })
    }

    /// Runs `f` with the account as the record holds it now, under the record's exclusive lock,
    /// and writes back the account `f` returns. A refresh runs this way, so two processes never
    /// spend one refresh token (research R6), and the lock is held until `f` returns, its vault
    /// write included.
    pub(crate) fn with_account_locked<T>(
        &self,
        connector: &str,
        origin: &str,
        f: impl FnOnce(Option<&Account>) -> Result<(T, Option<Account>), RuntimeError>,
    ) -> Result<T, RuntimeError> {
        self.change(|record| {
            let index = record
                .accounts
                .iter()
                .position(|account| account.connector == connector && account.origin == origin);
            let (value, updated) = f(index.map(|index| &record.accounts[index]))?;
            match (index, updated) {
                (Some(index), Some(updated)) => {
                    record.accounts[index] = updated;
                    Ok((value, true))
                }
                _ => Ok((value, false)),
            }
        })
    }

    /// The record, or `None` when there is none yet; reading creates nothing.
    fn read(&self) -> Result<Option<AccountsRecord>, RuntimeError> {
        match fs::metadata(&self.path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(damaged(&self.path, error)),
            Ok(metadata) if !metadata.is_file() => return Err(damaged(&self.path, "not a file")),
            Ok(_) => {}
        }
        let _lock = records::lock(&self.path, false).map_err(|(at, error)| damaged(&at, error))?;
        load(&self.path).map(Some)
    }

    /// Applies `apply` to the record under its exclusive lock, creating it when missing, and
    /// writes the result when `apply` says it changed it. A refusal leaves it as it was.
    fn change<T>(
        &self,
        apply: impl FnOnce(&mut AccountsRecord) -> Result<(T, bool), RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let _lock = records::lock(&self.path, true).map_err(|(at, error)| damaged(&at, error))?;
        let mut record = match fs::metadata(&self.path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => AccountsRecord {
                record_version: RECORD_VERSION,
                vault: random_hex()?,
                accounts: Vec::new(),
            },
            _ => load(&self.path)?,
        };
        let (value, changed) = apply(&mut record)?;
        if changed {
            let mut json =
                serde_json::to_vec_pretty(&record).expect("an accounts record always serializes");
            json.push(b'\n');
            records::write_atomic(&self.path, &json).map_err(|error| {
                RuntimeError::new(
                    ErrorKind::AccountNotConnected,
                    format!("cannot write the accounts record {}: {error}", self.path.display()),
                )
            })?;
        }
        Ok(value)
    }
}

/// Longest account name kept, in characters (research R13).
const MAX_NAME_CHARS: usize = 100;

/// The record as stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountsRecord {
    record_version: u64,
    /// 16 random bytes in hex, naming this record's entries in the vault.
    vault: String,
    accounts: Vec<Account>,
}

fn load(path: &Path) -> Result<AccountsRecord, RuntimeError> {
    let bytes = fs::read(path).map_err(|error| damaged(path, error))?;
    let record: AccountsRecord =
        serde_json::from_slice(&bytes).map_err(|error| damaged(path, error))?;
    if record.record_version != RECORD_VERSION {
        return Err(damaged(
            path,
            format!(
                "it is version {}, and this host reads version {RECORD_VERSION}",
                record.record_version
            ),
        ));
    }
    Ok(record)
}

/// A record that cannot be used connects nothing, and is never written over.
fn damaged(path: &Path, reason: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::AccountNotConnected,
        format!(
            "the accounts record {} cannot be used ({reason}); no account is connected until it is fixed or deleted",
            path.display()
        ),
    )
}

/// The account as the tool names it: its name and, when known, its id.
fn describe(account: &Account) -> String {
    match &account.service_id {
        Some(id) => format!("the account {} (id {id})", account.name),
        None if account.kind == AccountKind::Key => "a key".to_owned(),
        None => format!("the account {}", account.name),
    }
}

/// 16 random bytes from the system's random source, in hex.
pub(crate) fn random_hex() -> Result<String, RuntimeError> {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut bytes = [0u8; 16];
    SystemRandom::new().fill(&mut bytes).map_err(|_| {
        RuntimeError::new(
            ErrorKind::AccountNotConnected,
            "the system's random source is unavailable",
        )
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

impl Account {
    /// Whether the connector's file still exists (`catalog` always does).
    pub fn origin_exists(&self) -> bool {
        self.origin == "catalog" || Path::new(&self.origin).is_file()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;
    use crate::connector::Level;
    use crate::error::ErrorKind;

    const TOKEN: &str = "gho_5d2f8a1c3e7b9d0f2a4c6e8b0d2f4a6c8e0b2d4f";

    fn new_account(service_id: Option<&str>, name: &str) -> NewAccount {
        NewAccount {
            connector: "local/notes".to_owned(),
            origin: "/home/ada/notes.json".to_owned(),
            kind: AccountKind::Account,
            flow: Some("authorization-code".to_owned()),
            client: Some(Client {
                client_id: "dashboard-dev".to_owned(),
                from: "option".to_owned(),
            }),
            name: name.to_owned(),
            service_id: service_id.map(str::to_owned),
            access_asked: vec![Access {
                name: "notes".to_owned(),
                level: Level::Read,
            }],
            access_reported: vec!["notes:read".to_owned()],
            now_ms: 1_000,
            expires_at_ms: Some(3_601_000),
        }
    }

    fn key() -> NewAccount {
        NewAccount {
            kind: AccountKind::Key,
            flow: None,
            client: None,
            service_id: None,
            access_asked: Vec::new(),
            access_reported: Vec::new(),
            expires_at_ms: None,
            ..new_account(None, "key")
        }
    }

    fn new_store() -> (TempDir, AccountStore) {
        let dir = TempDir::new().unwrap();
        let store = AccountStore::open(dir.path());
        (dir, store)
    }

    fn ok(_: &Account) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn record(store: &AccountStore) -> Value {
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap()
    }

    #[test]
    fn the_record_follows_the_data_model() {
        let (_dir, store) = new_store();
        assert!(store.list().unwrap().is_empty());
        assert!(!store.path().exists(), "nothing is written until something is connected");
        let vault = store.vault_id().unwrap();
        assert_eq!(vault.len(), 32);
        assert!(vault.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(store.vault_id().unwrap(), vault, "the vault id never changes");

        let Connected::New(account) = store.connect(new_account(Some("1234567"), "ada"), ok).unwrap() else {
            panic!("a new account");
        };
        assert_eq!(account.account_id.len(), 32);
        let written = record(&store);
        assert_eq!(written["record_version"], json!(1));
        assert_eq!(written["vault"], json!(vault));
        assert_eq!(
            written["accounts"][0],
            json!({
                "connector": "local/notes",
                "origin": "/home/ada/notes.json",
                "kind": "account",
                "flow": "authorization-code",
                "client": { "client_id": "dashboard-dev", "from": "option" },
                "account_id": account.account_id,
                "name": "ada",
                "service_id": "1234567",
                "access_asked": [{ "name": "notes", "level": "read" }],
                "access_reported": ["notes:read"],
                "connected_at_ms": 1000,
                "refreshed_at_ms": 1000,
                "expires_at_ms": 3601000,
                "needs_connecting": null
            })
        );
        assert_eq!(store.get("local/notes", "/home/ada/notes.json").unwrap(), Some(account));
        assert_eq!(store.get("local/notes", "/elsewhere/notes.json").unwrap(), None);
    }

    #[test]
    fn a_newer_or_damaged_record_is_refused_and_left_alone() {
        let (_dir, store) = new_store();
        for text in [
            r#"{"record_version": 2, "vault": "00", "accounts": []}"#.to_owned(),
            "{".to_owned(),
        ] {
            fs::write(store.path(), &text).unwrap();
            assert!(store.list().is_err());
            assert!(store.connect(new_account(Some("1"), "ada"), ok).is_err());
            assert_eq!(fs::read_to_string(store.path()).unwrap(), text);
        }
    }

    #[test]
    fn a_long_name_is_cut_to_100_characters() {
        let (_dir, store) = new_store();
        let Connected::New(account) = store.connect(new_account(Some("1"), &"é".repeat(150)), ok).unwrap() else {
            panic!("a new account");
        };
        assert_eq!(account.name.chars().count(), 100);
    }

    /// Clarification 2 of the spec: the same account reconnects, keeping its account id; any
    /// other is refused, naming the current one, and nothing changes.
    #[test]
    fn only_the_same_account_connects_again() {
        let (_dir, store) = new_store();
        let Connected::New(first) = store.connect(new_account(Some("1234567"), "ada"), ok).unwrap() else {
            panic!("a new account");
        };
        let mut again = new_account(Some("1234567"), "ada-renamed");
        again.now_ms = 5_000;
        let Connected::Again(second) = store.connect(again, ok).unwrap() else {
            panic!("the same account again");
        };
        assert_eq!(second.account_id, first.account_id);
        assert_eq!((second.name.as_str(), second.connected_at_ms), ("ada-renamed", 5_000));

        let before = fs::read(store.path()).unwrap();
        for other in [new_account(Some("7654321"), "bob"), new_account(None, "account")] {
            let error = store.connect(other, |_| panic!("the vault is never written")).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::AnotherAccountConnected);
            assert!(error.detail().contains("ada-renamed"), "{error}");
            assert!(error.detail().contains("disconnect"), "{error}");
        }
        assert_eq!(fs::read(store.path()).unwrap(), before);

        // A key: never replaced while one is stored
        let (_dir, store) = new_store();
        store.connect(key(), ok).unwrap();
        let error = store.connect(key(), |_| panic!("the vault is never written")).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AnotherAccountConnected);
    }

    #[test]
    fn a_failed_vault_write_leaves_the_record_as_it_was() {
        let (_dir, store) = new_store();
        let error = RuntimeError::new(ErrorKind::KeychainUnavailable, "locked");
        assert_eq!(
            store.connect(new_account(Some("1"), "ada"), |_| Err(error.clone())).unwrap_err(),
            error
        );
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn disconnecting_deletes_the_vault_entry_then_the_account() {
        let (_dir, store) = new_store();
        store.connect(new_account(Some("1"), "ada"), ok).unwrap();
        let mut seen = None;
        let gone = store
            .disconnect("local/notes", "/home/ada/notes.json", |account| {
                // Still in the record while the vault entry is deleted
                seen = Some(account.name.clone());
                Ok(())
            })
            .unwrap()
            .expect("an account was connected");
        assert_eq!((gone.name.as_str(), seen.as_deref()), ("ada", Some("ada")));
        assert!(store.list().unwrap().is_empty());
        assert_eq!(
            store.disconnect("local/notes", "/home/ada/notes.json", |_| panic!("nothing to delete")).unwrap(),
            None
        );
        // A vault that cannot delete leaves the account connected
        store.connect(new_account(Some("1"), "ada"), ok).unwrap();
        let locked = RuntimeError::new(ErrorKind::KeychainUnavailable, "locked");
        assert!(store.disconnect("local/notes", "/home/ada/notes.json", |_| Err(locked.clone())).is_err());
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn an_account_is_marked_only_while_it_is_the_same_one() {
        let (_dir, store) = new_store();
        let Connected::New(account) = store.connect(new_account(Some("1"), "ada"), ok).unwrap() else {
            panic!("a new account");
        };
        store
            .mark_needs_connecting("local/notes", "/home/ada/notes.json", "another-id", Reason::Refused, 9)
            .unwrap();
        assert_eq!(store.list().unwrap()[0].needs_connecting, None);
        store
            .mark_needs_connecting("local/notes", "/home/ada/notes.json", &account.account_id, Reason::RefreshFailed, 9)
            .unwrap();
        assert_eq!(
            store.list().unwrap()[0].needs_connecting,
            Some(NeedsConnecting {
                reason: Reason::RefreshFailed,
                at_ms: 9
            })
        );
        assert_eq!(record(&store)["accounts"][0]["needs_connecting"]["reason"], json!("refresh-failed"));
    }

    /// Clarification 1: an account whose connector file is gone is still listed, and found.
    #[test]
    fn accounts_list_whether_their_connector_file_exists() {
        let (dir, store) = new_store();
        let file = dir.path().join("notes.json");
        fs::write(&file, "{}").unwrap();
        let mut here = new_account(Some("1"), "ada");
        here.origin = file.display().to_string();
        store.connect(here, ok).unwrap();
        store.connect(new_account(Some("2"), "bob"), ok).unwrap();
        let accounts = store.list().unwrap();
        let exists: Vec<(&str, bool)> = accounts
            .iter()
            .map(|account| (account.name.as_str(), account.origin_exists()))
            .collect();
        assert!(exists.contains(&("ada", true)) && exists.contains(&("bob", false)), "{exists:?}");
    }

    #[test]
    fn the_record_never_holds_a_credential() {
        let (_dir, store) = new_store();
        store
            .connect(new_account(Some("1"), "ada"), |_| {
                // The vault is where a token goes; nothing of it reaches the record
                let _ = TOKEN;
                Ok(())
            })
            .unwrap();
        let text = fs::read_to_string(store.path()).unwrap();
        assert!(!text.contains(TOKEN));
        assert!(!text.contains("access_token") && !text.contains("refresh_token"));
    }
}
