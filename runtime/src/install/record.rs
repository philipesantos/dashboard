//! The app's record of the plugins it installed, `<app data dir>/host/installed.json`
//! (`specs/010-plugin-installation/data-model.md`, Installed record): host data that no plugin
//! reaches, read and written under its lock file, replaced whole with an atomic rename, as the
//! grant record is.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::Repository;
use crate::error::{ErrorKind, RuntimeError};
use crate::records::{lock, write_atomic};

/// The format this host writes and reads.
pub const INSTALLED_VERSION: u64 = 1;

/// One installed plugin: where it came from, the release pinned, and its two files' digests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installed {
    /// GitHub's permanent identifier of the repository, kept from the first installation;
    /// updates are fetched by it only.
    pub repository_id: u64,
    /// The pinned release: its tag, and the version the tag and the manifest share.
    pub tag: String,
    pub version: String,
    /// `sha256:` of `package.wasm`; its hex names the package's directory.
    pub package_digest: String,
    /// `sha256:` of `compiled.cwasm`, checked before every `deserialize`.
    pub compiled_digest: String,
    /// When it was installed, as RFC 3339 in UTC.
    pub installed_at: String,
}

impl Installed {
    /// The grant record's origin for this plugin: its repository's permanent identifier, so a
    /// package claiming the id from any other repository gets none of its grants.
    pub fn origin(&self) -> String {
        origin(self.repository_id)
    }

    /// The directory, under `packages/`, of its package and compiled form.
    pub fn directory(&self) -> String {
        directory(&self.package_digest)
    }
}

/// `github:<repository_id>`, the grant record's origin for an installed plugin.
pub fn origin(repository_id: u64) -> String {
    format!("github:{repository_id}")
}

/// The hex of a `sha256:` digest, the name of a package's directory.
pub fn directory(package_digest: &str) -> String {
    package_digest
        .strip_prefix("sha256:")
        .unwrap_or(package_digest)
        .to_owned()
}

/// The record as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    record_version: u64,
    plugins: BTreeMap<String, Installed>,
}

impl Record {
    fn parse(bytes: &[u8]) -> Result<Record, String> {
        let record: Record = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if record.record_version != INSTALLED_VERSION {
            return Err(format!(
                "record_version {} is not {INSTALLED_VERSION}, the one this host reads",
                record.record_version
            ));
        }
        for (id, installed) in &record.plugins {
            if Repository::from_plugin_id(id).is_none() {
                return Err(format!("{id} is not a repository plugin id"));
            }
            for digest in [&installed.package_digest, &installed.compiled_digest] {
                if !is_digest(digest) {
                    return Err(format!("{id}: {digest} is not sha256:<64 hex digits>"));
                }
            }
        }
        Ok(record)
    }
}

fn is_digest(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    })
}

/// The installed record at one path. Nothing is read or written until it is used; a missing
/// file is an empty record.
#[derive(Debug, Clone)]
pub struct InstalledStore {
    path: PathBuf,
}

impl InstalledStore {
    pub fn open(path: impl Into<PathBuf>) -> InstalledStore {
        InstalledStore { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every installed plugin, by id. A record this host would not have written is an error,
    /// never an empty record: the caller then removes nothing (research R5).
    pub fn read(&self) -> Result<BTreeMap<String, Installed>, RuntimeError> {
        let _lock = match lock(&self.path, false) {
            Ok(lock) => lock,
            // No directory yet: nothing was ever installed
            Err((_, error)) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(BTreeMap::new());
            }
            Err((path, error)) => return Err(unreadable(&path, &error)),
        };
        Ok(self.load()?.plugins)
    }

    /// The plugin `id`, if installed.
    pub fn get(&self, id: &str) -> Result<Option<Installed>, RuntimeError> {
        Ok(self.read()?.remove(id))
    }

    /// Records `installed` as the plugin `id`, replacing any earlier entry.
    pub fn insert(&self, id: &str, installed: Installed) -> Result<(), RuntimeError> {
        if Repository::from_plugin_id(id).is_none() {
            return Err(RuntimeError::new(
                ErrorKind::InvalidManifest,
                format!("{id} is not a repository plugin id"),
            ));
        }
        self.change(|plugins| {
            plugins.insert(id.to_owned(), installed);
        })
    }

    /// Removes the plugin `id`; returns whether it was there.
    pub fn remove(&self, id: &str) -> Result<bool, RuntimeError> {
        let mut removed = false;
        self.change(|plugins| removed = plugins.remove(id).is_some())?;
        Ok(removed)
    }

    fn change(
        &self,
        apply: impl FnOnce(&mut BTreeMap<String, Installed>),
    ) -> Result<(), RuntimeError> {
        let _lock = lock(&self.path, true).map_err(|(path, error)| unreadable(&path, &error))?;
        let mut record = self.load()?;
        apply(&mut record.plugins);
        let bytes = serde_json::to_vec_pretty(&record).expect("the record serializes");
        write_atomic(&self.path, &bytes).map_err(|error| unwritable(&self.path, &error))
    }

    fn load(&self) -> Result<Record, RuntimeError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Record::parse(&bytes).map_err(|reason| {
                RuntimeError::new(
                    ErrorKind::NotGranted,
                    format!(
                        "the installed record {} cannot be read: {reason}",
                        self.path.display()
                    ),
                )
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Record {
                record_version: INSTALLED_VERSION,
                plugins: BTreeMap::new(),
            }),
            Err(error) => Err(unreadable(&self.path, &error)),
        }
    }
}

fn unreadable(path: &Path, error: &io::Error) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::NotGranted,
        format!(
            "the installed record {} cannot be read: {error}",
            path.display()
        ),
    )
}

fn unwritable(path: &Path, error: &io::Error) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::NotGranted,
        format!(
            "the installed record {} cannot be written: {error}",
            path.display()
        ),
    )
}
