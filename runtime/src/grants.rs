//! The grant record: every approval the user gave, kept by the host
//! (`specs/002-plugin-permissions/`, data-model.md and research R4).
//!
//! Plugins have no file access, so they can neither read nor change it (FR-009). In this phase
//! the only record is the development one, which the CLI changes for local package files.
//! Every approval belongs to the origin it was approved from, so another package that claims
//! the same id gets none of its grants (FR-020).
//!
//! Version 2 (`specs/003-plugin-connectors/data-model.md`) adds the connectors each approval
//! covers: where each came from, its destinations and the digests of the operations the widget
//! declares, so a change to any of them needs a new approval.
//!
//! Version 3 (`specs/004-plugin-storage/data-model.md`) adds the storage quota the last approval
//! showed, so a later version that declares more needs the user again.
//!
//! Version 4 (`specs/005-plugin-credentials/data-model.md`) adds, for each connector that declares
//! a credential, whether the widget uses its account, the credential's digest and the access
//! approved, so a new credential, more access or starting to use the account needs the user again.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use crate::error::{ErrorKind, RuntimeError};
use crate::connector::Access;
use crate::manifest::{Manifest, PluginId};
use crate::permissions;
use crate::records;

/// The record format this host writes. It also reads versions 1 to 3, as version 4 with no
/// connectors (version 1), no storage quota (versions 1 and 2) and no account (versions 1 to 3),
/// and writes them back as version 4.
pub const RECORD_VERSION: u64 = 4;
/// The format of feature 002, still read.
const RECORD_VERSION_1: u64 = 1;
/// The format of feature 003, still read.
const RECORD_VERSION_2: u64 = 2;
/// The format of feature 004, still read.
const RECORD_VERSION_3: u64 = 3;

/// The app's directory inside the system's data directory. Provisional: it moves with the
/// project's final name, before the first release.
pub(crate) const APP_DIR: &str = "dev.philsantos.dashboard";
/// The development record's file name.
const RECORD_FILE: &str = "development-grants.json";
/// The permission whose approval carries a quota.
const STORAGE: &str = "storage";

/// One plugin's approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Where the approved package came from: in the CLI, the canonical path of its file. The
    /// entry grants nothing to a package from any other origin.
    pub origin: String,
    /// Sorted and unique: the required permissions approved, plus the optional ones turned
    /// on. A name the host does not know is kept and never linked.
    pub granted: Vec<String>,
    /// The package last approved, for display only.
    pub approved_version: String,
    pub approved_digest: String,
    /// One entry per connector the approved manifest declared (record version 2).
    #[serde(default)]
    pub connectors: BTreeMap<String, ConnectorGrant>,
    /// The storage quota the last approval showed, in usage bytes; present only once an approved
    /// manifest declared storage (record version 3, `specs/004-plugin-storage/data-model.md`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_quota_bytes: Option<u64>,
}

/// What an approval covers of one connector (`data-model.md`, Grant record, version 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorGrant {
    /// `catalog`, or the canonical path of the local connector file.
    pub origin: String,
    /// The connector's version, for display; a new version that changes nothing the widget
    /// uses needs no approval.
    pub version: String,
    /// Required connectors: `true`. Optional ones: `true` only while on.
    pub granted: bool,
    /// Every origin of the connector, sorted.
    pub destinations: Vec<String>,
    /// For each operation the widget declares, the digest of its declaration.
    pub operations: BTreeMap<String, String>,
    /// Present only when the connector declares a credential (record version 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountGrant>,
}

/// What an approval covers of a connector's credential (`specs/005-plugin-credentials/
/// data-model.md`, Grant record, version 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountGrant {
    /// The manifest's `account` when approved: whether the widget acts as the account.
    pub uses: bool,
    /// The credential's digest, without its access.
    pub credential: String,
    /// The access approved, sorted by name.
    pub access: Vec<Access>,
}

impl Entry {
    pub fn is_granted(&self, permission: &str) -> bool {
        self.granted.iter().any(|name| name == permission)
    }

    /// Whether the connector `id` is granted to this approval.
    pub fn is_connector_granted(&self, id: &str) -> bool {
        self.connectors
            .get(id)
            .is_some_and(|connector| connector.granted)
    }

    /// Whether the connector `id` is granted and its account approved for use: the widget is
    /// linked to it through `accounts` only when its manifest also says `account: true`.
    pub fn uses_account(&self, id: &str) -> bool {
        self.connectors.get(id).is_some_and(|connector| {
            connector.granted && connector.account.as_ref().is_some_and(|account| account.uses)
        })
    }

    /// What a load of `manifest` needs approved again about storage: when storage is required,
    /// or optional and on, a declared quota above the approved one, or none approved yet.
    /// `None` when the approval covers it.
    pub fn storage_change(&self, manifest: &Manifest) -> Option<String> {
        let declared = manifest.storage()?;
        if !declared.required && !self.is_granted(STORAGE) {
            return None;
        }
        match self.storage_quota_bytes {
            Some(approved) if declared.quota_bytes <= approved => None,
            Some(approved) => Some(format!(
                "storage quota raised from {approved} to {} bytes",
                declared.quota_bytes
            )),
            None => Some(format!(
                "storage with a quota of {} bytes is not approved yet",
                declared.quota_bytes
            )),
        }
    }

    /// What changed in `connectors` since this approval, for each required connector and each
    /// optional one that is on: an empty list when the approval still covers them all. A
    /// connector that only added operations the widget does not declare changes nothing.
    pub fn connector_changes(
        &self,
        manifest: &Manifest,
        connectors: &[ConnectorState],
    ) -> Vec<String> {
        let mut changes = Vec::new();
        for state in connectors {
            let Some(request) = manifest.connector(&state.id) else {
                continue;
            };
            let Some(grant) = self.connectors.get(&state.id) else {
                changes.push(format!("connector newly declared: {}", state.id));
                continue;
            };
            if !request.required && !grant.granted {
                continue;
            }
            if grant.origin != state.origin {
                changes.push(format!(
                    "origin changed: {} (now {})",
                    state.id, state.origin
                ));
                continue;
            }
            for destination in &state.destinations {
                if !grant.destinations.contains(destination) {
                    changes.push(format!("destination added: {destination}"));
                }
            }
            for (operation, digest) in &state.operations {
                match grant.operations.get(operation) {
                    None => changes.push(format!("operation newly declared: {operation}")),
                    Some(approved) if approved != digest => {
                        changes.push(format!("operation changed: {operation}"));
                    }
                    Some(_) => {}
                }
            }
            if let Some(account) = &state.account {
                changes.extend(account_changes(&state.id, account, grant.account.as_ref()));
            }
        }
        changes
    }
}

/// A declared connector as loaded now: what an approval of it records and compares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorState {
    pub id: String,
    /// `catalog`, or the canonical path of the local connector file.
    pub origin: String,
    pub version: String,
    pub required: bool,
    /// Every origin of the connector, sorted.
    pub destinations: Vec<String>,
    /// Each operation the widget declares, with the digest of its declaration.
    pub operations: BTreeMap<String, String>,
    /// Present only when the connector declares a credential.
    pub account: Option<AccountState>,
}

/// A connector's credential as loaded now, and whether the widget declares that it uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountState {
    /// The manifest's `account`.
    pub uses: bool,
    /// The credential's own `required`: a required credential applies whether or not the widget
    /// says it uses it, and loading refuses a widget that does not.
    pub required: bool,
    /// The credential's digest, without its access.
    pub credential: String,
    /// The access the connector asks for now, sorted.
    pub access: Vec<Access>,
}

/// What changed in a connector's credential since its approval (research R8): nothing when the
/// widget neither uses the account nor needs it.
fn account_changes(id: &str, now: &AccountState, approved: Option<&AccountGrant>) -> Vec<String> {
    if !now.uses && !now.required {
        return Vec::new();
    }
    let Some(approved) = approved else {
        return vec![format!("credential added: {id}")];
    };
    let mut changes = Vec::new();
    if approved.credential != now.credential {
        changes.push(format!("credential changed: {id}"));
    }
    if now.uses && !approved.uses {
        changes.push(format!("account use newly declared: {id}"));
    }
    for item in &now.access {
        let covered = approved
            .access
            .iter()
            .any(|kept| kept.name == item.name && kept.level >= item.level);
        if !covered {
            changes.push(format!(
                "access added: {id} {} {}",
                item.name,
                item.level.as_str()
            ));
        }
    }
    changes
}

/// The approval digest (`data-model.md`, Approval digest): `sha256:` and the hex SHA-256 of a
/// canonical JSON of the package digest, the manifest's permissions, each declared connector's
/// id, origin, version, `required`, destinations and operation digests, and the storage
/// declaration when there is one. What
/// `permissions` shows and `approve` grants carry the same one.
pub fn approval_digest(
    package_digest: &str,
    manifest: &Manifest,
    connectors: &[ConnectorState],
) -> String {
    let mut permissions: Vec<serde_json::Value> = manifest
        .permissions()
        .iter()
        .map(|permission| serde_json::json!({ "name": permission.name, "required": permission.required }))
        .collect();
    permissions.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let mut connectors: Vec<&ConnectorState> = connectors.iter().collect();
    connectors.sort_by(|a, b| a.id.cmp(&b.id));
    let connectors: Vec<serde_json::Value> = connectors
        .into_iter()
        .map(|state| {
            let mut connector = serde_json::json!({
                "id": state.id,
                "origin": state.origin,
                "version": state.version,
                "required": state.required,
                "destinations": state.destinations,
                "operations": state.operations,
            });
            // Only when the connector declares a credential, so widgets whose connectors declare
            // none keep their digests (`specs/005-plugin-credentials/data-model.md`)
            if let Some(account) = &state.account {
                connector["account"] = serde_json::json!({
                    "uses": account.uses,
                    "credential": account.credential,
                    "access": account.access,
                });
            }
            connector
        })
        .collect();
    let mut document = serde_json::json!({
        "package": package_digest,
        "permissions": permissions,
        "connectors": connectors,
    });
    // Only when declared, so a widget without storage keeps the digest, and the kept data, it
    // had (`specs/004-plugin-storage/data-model.md`, Approval digest)
    if let Some(storage) = manifest.storage() {
        document["storage"] = serde_json::json!({
            "required": storage.required,
            "quota_bytes": storage.quota_bytes,
        });
    }
    crate::connector::sha256(crate::connector::canonical(&document).as_bytes())
}

/// What [`GrantStore::approve`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    /// The required permissions granted, in the manifest's order.
    pub required: Vec<String>,
    /// The origin of the approval this one replaced, when it came from another origin.
    pub replaced: Option<String>,
}

/// What a package requests and what its approval grants, for display (FR-014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Each permission the manifest requests, in its order.
    pub permissions: Vec<PermissionStatus>,
    /// The plugin's approval, whatever its origin.
    pub approval: Option<Entry>,
    /// Whether the approval is from this package's origin. One from another origin grants
    /// this package nothing.
    pub this_origin: bool,
}

/// One permission a package requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionStatus {
    pub name: String,
    pub required: bool,
    /// Granted to packages from this origin.
    pub granted: bool,
}

/// The record as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantRecord {
    record_version: u64,
    plugins: BTreeMap<String, Entry>,
}

impl GrantRecord {
    fn empty() -> GrantRecord {
        GrantRecord {
            record_version: RECORD_VERSION,
            plugins: BTreeMap::new(),
        }
    }

    /// Reads a record, refusing anything this host would not have written.
    fn parse(bytes: &[u8]) -> Result<GrantRecord, String> {
        // The version first, so a newer record is named as such, whatever else it holds
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| format!("not JSON: {error}"))?;
        let version = match value
            .get("record_version")
            .and_then(serde_json::Value::as_u64)
        {
            Some(version @ (RECORD_VERSION_1 | RECORD_VERSION_2 | RECORD_VERSION_3 | RECORD_VERSION)) => {
                version
            }
            Some(version) if version > RECORD_VERSION => {
                return Err(format!(
                    "record_version {version} is newer than this host's {RECORD_VERSION}"
                ));
            }
            _ => {
                return Err(format!(
                    "record_version must be {RECORD_VERSION_1} to {RECORD_VERSION}"
                ));
            }
        };
        // Parsed again from the bytes, which refuses duplicate fields
        let mut record: GrantRecord =
            serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        for (id, entry) in &record.plugins {
            PluginId::parse(id)?;
            if !entry.granted.windows(2).all(|pair| pair[0] < pair[1]) {
                return Err(format!("the grants of {id} are not sorted and unique"));
            }
            if version == RECORD_VERSION_1 && !entry.connectors.is_empty() {
                return Err(format!(
                    "{id} has connectors in a record of version {RECORD_VERSION_1}, which has none"
                ));
            }
            if version < RECORD_VERSION_3 && entry.storage_quota_bytes.is_some() {
                return Err(format!(
                    "{id} has a storage quota in a record of version {version}, which has none"
                ));
            }
            for (connector, grant) in &entry.connectors {
                if !grant.destinations.windows(2).all(|pair| pair[0] < pair[1]) {
                    return Err(format!(
                        "the destinations of {connector} in {id} are not sorted and unique"
                    ));
                }
                if version < RECORD_VERSION && grant.account.is_some() {
                    return Err(format!(
                        "{connector} in {id} has an account in a record of version {version}, which has none"
                    ));
                }
            }
        }
        // Older records are version 4 without what they did not have, written back as 4
        record.record_version = RECORD_VERSION;
        Ok(record)
    }
}

/// The runtime's view of one grant record: where it lives, and a generation that every change
/// through this store increments, so a loaded plugin knows to link again (research R6).
pub struct GrantStore {
    backing: Backing,
    generation: AtomicU64,
}

enum Backing {
    /// For tests and examples: a record that lives as long as the store.
    Memory(Mutex<GrantRecord>),
    /// A JSON file, read on every use and changed under its lock file.
    File(PathBuf),
}

impl GrantStore {
    /// A store whose record lives in memory, empty at first.
    pub fn in_memory() -> GrantStore {
        GrantStore {
            backing: Backing::Memory(Mutex::new(GrantRecord::empty())),
            generation: AtomicU64::new(0),
        }
    }

    /// A store backed by the record at `path`. Nothing is read or written until it is used; a
    /// missing file counts as an empty record.
    pub fn open(path: impl Into<PathBuf>) -> GrantStore {
        GrantStore {
            backing: Backing::File(path.into()),
            generation: AtomicU64::new(0),
        }
    }

    /// The development record in the system's data directory, if the system has one.
    pub fn default_development_path() -> Option<PathBuf> {
        dirs::data_dir().map(|dir| dir.join(APP_DIR).join(RECORD_FILE))
    }

    /// The record's file, for a store backed by one.
    pub fn path(&self) -> Option<&Path> {
        match &self.backing {
            Backing::Memory(_) => None,
            Backing::File(path) => Some(path),
        }
    }

    /// Increases with every change made through this store.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Whether `permission` is granted to the plugin's approval from `origin`, read from the
    /// record as it is now, under its lock: a change made by any process applies.
    pub fn permission_granted(
        &self,
        plugin: &PluginId,
        origin: &str,
        permission: &str,
    ) -> Result<bool, RuntimeError> {
        Ok(self
            .entry(plugin)?
            .is_some_and(|entry| entry.origin == origin && entry.is_granted(permission)))
    }

    /// Runs `commit` only while the record, read under its shared lock, grants `permission` to
    /// the plugin's approval from `origin`, and holds the lock until `commit` returns, so no
    /// change to the grants falls between the check and what `commit` does (research R7).
    /// `None` when the permission is not granted, and `commit` did not run.
    pub fn while_granted<T>(
        &self,
        plugin: &PluginId,
        origin: &str,
        permission: &str,
        commit: impl FnOnce() -> T,
    ) -> Result<Option<T>, RuntimeError> {
        let granted = |record: &GrantRecord| {
            record
                .plugins
                .get(plugin.as_str())
                .is_some_and(|entry| entry.origin == origin && entry.is_granted(permission))
        };
        match &self.backing {
            Backing::Memory(record) => {
                let record = record.lock().unwrap_or_else(PoisonError::into_inner);
                Ok(granted(&record).then(commit))
            }
            Backing::File(path) => {
                match fs::metadata(path) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => return Err(damaged(path, error)),
                    Ok(_) => {}
                }
                // Held until `commit` returns: a change to the record waits for it
                let _lock = lock(path, false)?;
                let record = load(path)?;
                Ok(granted(&record).then(commit))
            }
        }
    }

    /// The plugin's approval, if it has one, whatever its origin. Refused as `not-granted` when
    /// the record cannot be used.
    pub fn entry(&self, id: &PluginId) -> Result<Option<Entry>, RuntimeError> {
        Ok(self.read()?.plugins.remove(id.as_str()))
    }

    /// What the manifest requests and what the plugin's approval grants to packages from
    /// `origin`. Refused as `not-granted` when the record cannot be used.
    pub fn status(&self, manifest: &Manifest, origin: &str) -> Result<Status, RuntimeError> {
        let approval = self.entry(manifest.id())?;
        let this_origin = approval
            .as_ref()
            .is_some_and(|entry| entry.origin == origin);
        let permissions = manifest
            .permissions()
            .iter()
            .map(|permission| PermissionStatus {
                name: permission.name.clone(),
                required: permission.required,
                granted: this_origin
                    && approval
                        .as_ref()
                        .is_some_and(|entry| entry.is_granted(&permission.name)),
            })
            .collect();
        Ok(Status {
            permissions,
            approval,
            this_origin,
        })
    }

    /// Approves the manifest's required permissions, all together, for packages from `origin`
    /// (FR-018). From the entry's own origin, `granted` becomes (`granted` ∩ declared) ∪
    /// required. With no entry, or one from another origin, the entry is replaced: only the
    /// required permissions are granted (FR-020).
    pub fn approve(
        &self,
        manifest: &Manifest,
        origin: &str,
        digest: &str,
    ) -> Result<Approval, RuntimeError> {
        if manifest.storage().is_some() {
            return Err(RuntimeError::new(
                ErrorKind::ApprovalChanged,
                "a plugin that declares storage is approved only with the approval digest of what was shown, its quota included",
            ));
        }
        let required: Vec<String> = manifest
            .permissions()
            .iter()
            .filter(|permission| permission.required)
            .map(|permission| permission.name.clone())
            .collect();
        self.change(|record| {
            let (mut granted, replaced) = match record.plugins.get(manifest.id().as_str()) {
                Some(entry) if entry.origin == origin => {
                    let declared = entry
                        .granted
                        .iter()
                        .filter(|name| manifest.permission(name).is_some())
                        .cloned()
                        .collect();
                    (declared, None)
                }
                Some(entry) => (Vec::new(), Some(entry.origin.clone())),
                None => (Vec::new(), None),
            };
            granted.extend(required.iter().cloned());
            granted.sort();
            granted.dedup();
            record.plugins.insert(
                manifest.id().to_string(),
                Entry {
                    origin: origin.to_owned(),
                    granted,
                    approved_version: manifest.version().to_owned(),
                    approved_digest: digest.to_owned(),
                    connectors: BTreeMap::new(),
                    storage_quota_bytes: None,
                },
            );
            let approval = Approval {
                required: required.clone(),
                replaced,
            };
            Ok((approval, true))
        })
    }

    /// Approves a widget and every connector it declares, as [`approve`](Self::approve) does
    /// its permissions, only when `expected` is the [`approval_digest`] of what it would approve
    /// now: anything that changed since it was shown is refused as `approval-changed`, granting
    /// nothing (FR-014). Each declared connector is recorded with its origin, destinations and
    /// operation digests; required ones are granted, and an optional one keeps its state when
    /// nothing about it changed, and is off otherwise. A plugin that declares storage is
    /// approved here, with the quota shown recorded (feature 004).
    pub fn approve_connected(
        &self,
        manifest: &Manifest,
        origin: &str,
        digest: &str,
        connectors: &[ConnectorState],
        expected: &str,
    ) -> Result<Approval, RuntimeError> {
        let current = approval_digest(digest, manifest, connectors);
        if expected != current {
            return Err(RuntimeError::new(
                ErrorKind::ApprovalChanged,
                format!(
                    "what would be approved now has the digest {current}, not {expected}: the package, its permissions or a connector changed since it was shown; show it again"
                ),
            ));
        }
        let required: Vec<String> = manifest
            .permissions()
            .iter()
            .filter(|permission| permission.required)
            .map(|permission| permission.name.clone())
            .collect();
        self.change(|record| {
            let previous = record.plugins.get(manifest.id().as_str()).cloned();
            let same_origin = previous.as_ref().filter(|entry| entry.origin == origin);
            let mut granted: Vec<String> = same_origin
                .map(|entry| {
                    entry
                        .granted
                        .iter()
                        .filter(|name| manifest.permission(name).is_some())
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            granted.extend(required.iter().cloned());
            granted.sort();
            granted.dedup();
            let mut grants = BTreeMap::new();
            for state in connectors {
                let Some(request) = manifest.connector(&state.id) else {
                    continue;
                };
                let account = state.account.as_ref().map(|account| AccountGrant {
                    uses: account.uses,
                    credential: account.credential.clone(),
                    access: account.access.clone(),
                });
                let kept_on = same_origin
                    .and_then(|entry| entry.connectors.get(&state.id))
                    .is_some_and(|grant| {
                        grant.granted
                            && grant.origin == state.origin
                            && grant.destinations == state.destinations
                            && grant.operations == state.operations
                            && grant.account == account
                    });
                grants.insert(
                    state.id.clone(),
                    ConnectorGrant {
                        origin: state.origin.clone(),
                        version: state.version.clone(),
                        granted: request.required || kept_on,
                        destinations: state.destinations.clone(),
                        operations: state.operations.clone(),
                        account,
                    },
                );
            }
            record.plugins.insert(
                manifest.id().to_string(),
                Entry {
                    origin: origin.to_owned(),
                    granted,
                    approved_version: manifest.version().to_owned(),
                    approved_digest: digest.to_owned(),
                    connectors: grants,
                    // The quota this approval showed, smaller or larger than before
                    storage_quota_bytes: manifest.storage().map(|storage| storage.quota_bytes),
                },
            );
            let replaced = previous
                .filter(|entry| entry.origin != origin)
                .map(|entry| entry.origin);
            Ok((
                Approval {
                    required: required.clone(),
                    replaced,
                },
                true,
            ))
        })
    }

    /// Turns one optional connector on or off. Refused as `not-declared` when the manifest does
    /// not declare it, `required-together` when it is required, and `not-granted` when the
    /// plugin has no approval from `origin` that covers it.
    pub fn set_connector(
        &self,
        manifest: &Manifest,
        origin: &str,
        id: &str,
        on: bool,
    ) -> Result<(), RuntimeError> {
        let names = || vec![id.to_owned()];
        match manifest.connector(id) {
            None => {
                return Err(RuntimeError::with_names(
                    ErrorKind::NotDeclared,
                    names(),
                    Some("the manifest does not declare this connector"),
                ));
            }
            Some(request) if request.required => {
                return Err(RuntimeError::with_names(
                    ErrorKind::RequiredTogether,
                    names(),
                    Some(
                        "it is required: required connectors are approved together, and change only with a new approval or by removing it",
                    ),
                ));
            }
            Some(_) => {}
        }
        self.change(|record| {
            let entry = approved_entry(record, manifest.id(), origin)?;
            let Some(grant) = entry.connectors.get_mut(id) else {
                return Err(RuntimeError::with_names(
                    ErrorKind::NotGranted,
                    names(),
                    Some("the approval does not cover this connector; approve the widget again"),
                ));
            };
            if grant.granted == on {
                return Ok(((), false));
            }
            grant.granted = on;
            Ok(((), true))
        })
    }

    /// Whether the connector `id` is granted to the plugin's approval from `origin`, read from
    /// the record as it is now, under its lock: a change made by any process applies.
    pub fn connector_granted(
        &self,
        plugin: &PluginId,
        origin: &str,
        id: &str,
    ) -> Result<bool, RuntimeError> {
        Ok(self
            .entry(plugin)?
            .is_some_and(|entry| entry.origin == origin && entry.is_connector_granted(id)))
    }

    /// Rewrites the version of each connector whose new version changes nothing the approval
    /// covers (load step 7). Writes nothing when no version moved.
    pub fn refresh_connector_versions(
        &self,
        manifest: &Manifest,
        origin: &str,
        connectors: &[ConnectorState],
    ) -> Result<(), RuntimeError> {
        self.change(|record| {
            let Some(entry) = record
                .plugins
                .get_mut(manifest.id().as_str())
                .filter(|entry| entry.origin == origin)
            else {
                return Ok(((), false));
            };
            let mut changed = false;
            for state in connectors {
                if let Some(grant) = entry.connectors.get_mut(&state.id)
                    && grant.version != state.version
                {
                    grant.version = state.version.clone();
                    changed = true;
                }
            }
            Ok(((), changed))
        })
    }

    /// Turns one optional permission on or off. Refused as `unknown-permission` when the host
    /// does not know it, `not-declared` when the manifest does not request it,
    /// `required-together` when it is required, and `not-granted` when the plugin has no
    /// approval from `origin`.
    pub fn set_optional(
        &self,
        manifest: &Manifest,
        origin: &str,
        permission: &str,
        on: bool,
    ) -> Result<(), RuntimeError> {
        let names = || vec![permission.to_owned()];
        if permissions::by_name(permission).is_none() {
            return Err(RuntimeError::with_names(
                ErrorKind::UnknownPermission,
                names(),
                Some("this host does not know it"),
            ));
        }
        match manifest.permission(permission) {
            None => {
                return Err(RuntimeError::with_names(
                    ErrorKind::NotDeclared,
                    names(),
                    Some("the manifest does not request it"),
                ));
            }
            Some(request) if request.required => {
                return Err(RuntimeError::with_names(
                    ErrorKind::RequiredTogether,
                    names(),
                    Some(
                        "it is required: required permissions are approved together, and change only with a new approval or by removing it",
                    ),
                ));
            }
            Some(_) => {}
        }
        self.change(|record| {
            let entry = approved_entry(record, manifest.id(), origin)?;
            if entry.is_granted(permission) == on {
                return Ok(((), false));
            }
            // Storage turned on with a quota the user did not approve needs the user again
            if on
                && permission == STORAGE
                && let Some(declared) = manifest.storage()
                && entry
                    .storage_quota_bytes
                    .is_none_or(|approved| declared.quota_bytes > approved)
            {
                let approved = entry.storage_quota_bytes.map_or_else(
                    || "no storage quota is approved".to_owned(),
                    |approved| format!("the approved quota is {approved} bytes"),
                );
                return Err(RuntimeError::with_names(
                    ErrorKind::NeedsNewApproval,
                    names(),
                    Some(&format!(
                        "it declares a quota of {} bytes and {approved}; approve the plugin again",
                        declared.quota_bytes
                    )),
                ));
            }
            if on {
                entry.granted.push(permission.to_owned());
                entry.granted.sort();
            } else {
                entry.granted.retain(|name| name != permission);
            }
            Ok(((), true))
        })
    }

    /// Removes the plugin's approval and every grant with it. Returns whether there was one.
    pub fn remove(&self, id: &PluginId) -> Result<bool, RuntimeError> {
        self.change(|record| {
            let removed = record.plugins.remove(id.as_str()).is_some();
            Ok((removed, removed))
        })
    }

    /// Removes from the plugin's approval every permission this manifest no longer declares
    /// (FR-012), when the approval is from `origin`. Returns the names removed; writes nothing
    /// when there are none.
    pub fn prune(&self, manifest: &Manifest, origin: &str) -> Result<Vec<String>, RuntimeError> {
        self.change(|record| {
            let Some(entry) = record
                .plugins
                .get_mut(manifest.id().as_str())
                .filter(|entry| entry.origin == origin)
            else {
                return Ok((Vec::new(), false));
            };
            let (kept, mut removed): (Vec<String>, Vec<String>) = entry
                .granted
                .drain(..)
                .partition(|name| manifest.permission(name).is_some());
            entry.granted = kept;
            // Connectors too, so declaring one again in a later version needs a new approval
            let dropped: Vec<String> = entry
                .connectors
                .keys()
                .filter(|id| manifest.connector(id).is_none())
                .cloned()
                .collect();
            for id in &dropped {
                entry.connectors.remove(id);
            }
            removed.extend(dropped);
            let changed = !removed.is_empty();
            Ok((removed, changed))
        })
    }

    /// The record as it is now.
    fn read(&self) -> Result<GrantRecord, RuntimeError> {
        match &self.backing {
            Backing::Memory(record) => Ok(record
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()),
            Backing::File(path) => {
                // A record that does not exist is empty, and reading it creates nothing
                match fs::metadata(path) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Ok(GrantRecord::empty());
                    }
                    Err(error) => return Err(damaged(path, error)),
                    Ok(metadata) if !metadata.is_file() => return Err(damaged(path, "not a file")),
                    Ok(_) => {}
                }
                let _lock = lock(path, false)?;
                load(path)
            }
        }
    }

    /// Applies `apply` to the record under its lock, and writes the result when `apply` says it
    /// changed it. A refusal, or a record that cannot be used, leaves it as it was.
    fn change<T>(
        &self,
        apply: impl FnOnce(&mut GrantRecord) -> Result<(T, bool), RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let (value, changed) = match &self.backing {
            Backing::Memory(record) => {
                let mut record = record.lock().unwrap_or_else(PoisonError::into_inner);
                let mut changed_record = record.clone();
                let (value, changed) = apply(&mut changed_record)?;
                if changed {
                    *record = changed_record;
                }
                (value, changed)
            }
            Backing::File(path) => {
                // Read, change and write under one exclusive lock, so an update made by another
                // process in between is never lost (research R4).
                let _lock = lock(path, true)?;
                let mut record = load(path)?;
                let (value, changed) = apply(&mut record)?;
                if changed {
                    write(path, &record)?;
                }
                (value, changed)
            }
        };
        if changed {
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
        Ok(value)
    }
}

/// The plugin's entry, if it was approved from `origin`; otherwise a `not-granted` error that
/// says why.
fn approved_entry<'a>(
    record: &'a mut GrantRecord,
    id: &PluginId,
    origin: &str,
) -> Result<&'a mut Entry, RuntimeError> {
    match record.plugins.get_mut(id.as_str()) {
        Some(entry) if entry.origin == origin => Ok(entry),
        Some(entry) => Err(RuntimeError::new(
            ErrorKind::NotGranted,
            format!(
                "{id} is approved from {}, not from {origin}; approve this package to replace that approval",
                entry.origin
            ),
        )),
        None => Err(RuntimeError::new(
            ErrorKind::NotGranted,
            format!("{id} has no approval; approve it first"),
        )),
    }
}

/// Takes the record's lock file, `<record>.lock`: shared to read, exclusive to change. The lock
/// is released when the returned file is dropped.
fn lock(path: &Path, exclusive: bool) -> Result<File, RuntimeError> {
    records::lock(path, exclusive).map_err(|(at, error)| {
        if at == path {
            unwritable(path, error)
        } else {
            damaged(&at, error)
        }
    })
}

/// Reads the record at `path`, under a lock the caller holds.
fn load(path: &Path) -> Result<GrantRecord, RuntimeError> {
    match fs::read(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(GrantRecord::empty()),
        Err(error) => Err(damaged(path, error)),
        Ok(bytes) => GrantRecord::parse(&bytes).map_err(|reason| damaged(path, reason)),
    }
}

/// Writes the record beside `path`, syncs it, then renames it over `path`, so a crash leaves
/// either the old record or the new one, never part of each.
fn write(path: &Path, record: &GrantRecord) -> Result<(), RuntimeError> {
    let mut json = serde_json::to_vec_pretty(record).expect("a grant record always serializes");
    json.push(b'\n');
    records::write_atomic(path, &json).map_err(|error| unwritable(path, error))
}

/// A record that cannot be used grants nothing, and is never written over (research R4).
fn damaged(path: &Path, reason: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::NotGranted,
        format!(
            "the grant record {} cannot be used ({reason}); it grants nothing until it is fixed or deleted",
            path.display()
        ),
    )
}

fn unwritable(path: &Path, error: io::Error) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::NotGranted,
        format!("cannot write the grant record {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::thread;

    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;

    const ORIGIN: &str = "/plugins/probe.wasm";
    const OTHER_ORIGIN: &str = "/downloads/probe.wasm";
    const DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    /// A manifest for `id` with `log` required, optional or not declared.
    fn manifest(id: &str, version: &str, log: Option<bool>) -> Manifest {
        let permissions: Vec<Value> = log
            .map(|required| json!({ "name": "log", "required": required }))
            .into_iter()
            .collect();
        let manifest = json!({
            "manifest_version": 1,
            "id": id,
            "version": version,
            "author": "A",
            "permissions": permissions,
            "login_providers": []
        });
        Manifest::parse(&serde_json::to_vec(&manifest).expect("serializes")).expect("valid")
    }

    fn id(id: &str) -> PluginId {
        PluginId::parse(id).expect("a valid id")
    }

    /// A file-backed store in a fresh directory, which the store must create.
    fn file_store() -> (TempDir, GrantStore) {
        let dir = TempDir::new().expect("a temporary directory");
        let store = GrantStore::open(dir.path().join("nested").join("grants.json"));
        (dir, store)
    }

    /// Runs `check` on an in-memory store and on a file-backed one.
    fn for_each_store(check: impl Fn(&GrantStore)) {
        check(&GrantStore::in_memory());
        let (_dir, store) = file_store();
        check(&store);
    }

    fn entry(store: &GrantStore, plugin: &str) -> Option<Entry> {
        store.entry(&id(plugin)).expect("the record is readable")
    }

    fn granted(store: &GrantStore, plugin: &str) -> Vec<String> {
        entry(store, plugin)
            .map(|entry| entry.granted)
            .unwrap_or_default()
    }

    /// A file store over a record with this exact text.
    fn store_with_text(text: &str) -> (TempDir, GrantStore, PathBuf) {
        let dir = TempDir::new().expect("a temporary directory");
        let path = dir.path().join("grants.json");
        fs::write(&path, text).expect("the record is written");
        (dir, GrantStore::open(&path), path)
    }

    #[test]
    fn writes_the_record_format_of_the_data_model() {
        let (_dir, store) = file_store();
        store
            .approve(
                &manifest("local/probe", "1.2.0", Some(true)),
                ORIGIN,
                DIGEST,
            )
            .expect("approved");
        let path = store.path().expect("a file store");
        let record: Value =
            serde_json::from_slice(&fs::read(path).expect("written")).expect("JSON");
        assert_eq!(
            record,
            json!({
                "record_version": 4,
                "plugins": {
                    "local/probe": {
                        "origin": ORIGIN,
                        "granted": ["log"],
                        "approved_version": "1.2.0",
                        "approved_digest": DIGEST,
                        "connectors": {}
                    }
                }
            })
        );
    }

    #[test]
    fn the_default_record_is_in_the_data_directory() {
        let path = GrantStore::default_development_path();
        match dirs::data_dir() {
            Some(data_dir) => {
                let path = path.expect("a path when the system has a data directory");
                assert!(path.starts_with(data_dir), "{}", path.display());
                assert!(
                    path.ends_with(
                        Path::new("dev.philsantos.dashboard").join("development-grants.json")
                    ),
                    "{}",
                    path.display()
                );
            }
            None => assert_eq!(path, None),
        }
    }

    #[test]
    fn a_missing_record_is_empty() {
        let (_dir, store) = file_store();
        assert_eq!(entry(&store, "local/probe"), None);
        assert!(
            !store.path().expect("a file store").exists(),
            "reading wrote nothing"
        );
        assert_eq!(GrantStore::in_memory().entry(&id("local/probe")), Ok(None));
    }

    #[test]
    fn reads_a_record_with_a_permission_the_host_does_not_know() {
        let (_dir, store, _path) = store_with_text(&format!(
            r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log","telepathy"],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
        ));
        assert_eq!(granted(&store, "local/probe"), ["log", "telepathy"]);
    }

    #[test]
    fn a_damaged_record_grants_nothing_and_is_never_overwritten() {
        let damaged = [
            ("not JSON", "{".to_owned()),
            ("not an object", "[]".to_owned()),
            (
                "a newer record version",
                r#"{"record_version":5,"plugins":{},"more":1}"#.to_owned(),
            ),
            (
                "an older record version",
                r#"{"record_version":0,"plugins":{}}"#.to_owned(),
            ),
            (
                "an unknown field",
                r#"{"record_version":1,"plugins":{},"x":1}"#.to_owned(),
            ),
            (
                "a malformed id",
                format!(
                    r#"{{"record_version":1,"plugins":{{"Local/Probe":{{"origin":"{ORIGIN}","granted":[],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
                ),
            ),
            (
                "unsorted grants",
                format!(
                    r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log","a"],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
                ),
            ),
            (
                "repeated grants",
                format!(
                    r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log","log"],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
                ),
            ),
        ];
        for (case, text) in damaged {
            let (_dir, store, path) = store_with_text(&text);
            let error = store.entry(&id("local/probe")).expect_err(case);
            assert_eq!(error.kind(), ErrorKind::NotGranted, "{case}: {error}");
            assert!(
                error.detail().contains("grants.json"),
                "{case}: the detail `{}` names no path",
                error.detail()
            );

            let probe = manifest("local/probe", "1.0.0", Some(true));
            let optional = manifest("local/probe", "1.0.0", Some(false));
            let refusals = [
                store.approve(&probe, ORIGIN, DIGEST).map(drop),
                store.set_optional(&optional, ORIGIN, "log", true),
                store.remove(&id("local/probe")).map(drop),
                store.prune(&probe, ORIGIN).map(drop),
            ];
            for refusal in refusals {
                assert_eq!(
                    refusal.map_err(|error| error.kind()),
                    Err(ErrorKind::NotGranted),
                    "{case}"
                );
            }
            assert_eq!(
                fs::read_to_string(&path).expect("still there"),
                text,
                "{case}"
            );
            assert_eq!(store.generation(), 0, "{case}");
        }
    }

    #[test]
    fn a_record_that_cannot_be_read_grants_nothing() {
        let dir = TempDir::new().expect("a temporary directory");
        // A directory where the file should be
        let store = GrantStore::open(dir.path());
        let error = store.entry(&id("local/probe")).expect_err("unreadable");
        assert_eq!(error.kind(), ErrorKind::NotGranted);
    }

    #[test]
    fn approve_grants_every_required_permission() {
        for_each_store(|store| {
            let approval = store
                .approve(
                    &manifest("local/probe", "1.0.0", Some(true)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            assert_eq!(
                approval,
                Approval {
                    required: vec!["log".to_owned()],
                    replaced: None
                }
            );
            assert_eq!(
                entry(store, "local/probe"),
                Some(Entry {
                    origin: ORIGIN.to_owned(),
                    granted: vec!["log".to_owned()],
                    approved_version: "1.0.0".to_owned(),
                    approved_digest: DIGEST.to_owned(),
                    connectors: BTreeMap::new(),
                    storage_quota_bytes: None,
                })
            );
        });
    }

    #[test]
    fn approve_leaves_optional_permissions_off() {
        for_each_store(|store| {
            let approval = store
                .approve(
                    &manifest("local/probe", "1.0.0", Some(false)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            assert!(approval.required.is_empty());
            assert_eq!(granted(store, "local/probe"), Vec::<String>::new());
            assert!(
                entry(store, "local/probe").is_some(),
                "approved with nothing granted"
            );
        });
    }

    #[test]
    fn approve_from_the_same_origin_keeps_what_is_still_declared() {
        for_each_store(|store| {
            let optional = manifest("local/probe", "1.0.0", Some(false));
            store.approve(&optional, ORIGIN, DIGEST).expect("approved");
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("turned on");

            // Still declared, still optional: stays on
            store
                .approve(
                    &manifest("local/probe", "1.1.0", Some(false)),
                    ORIGIN,
                    "sha256:1",
                )
                .expect("approved again");
            assert_eq!(granted(store, "local/probe"), ["log"]);
            let entry = entry(store, "local/probe").expect("approved");
            assert_eq!(entry.approved_version, "1.1.0");
            assert_eq!(entry.approved_digest, "sha256:1");

            // No longer declared: dropped
            store
                .approve(&manifest("local/probe", "1.2.0", None), ORIGIN, DIGEST)
                .expect("approved again");
            assert_eq!(granted(store, "local/probe"), Vec::<String>::new());
        });
    }

    #[test]
    fn approve_drops_undeclared_names_the_host_does_not_know() {
        let (_dir, store, _path) = store_with_text(&format!(
            r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log","telepathy"],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
        ));
        store
            .approve(
                &manifest("local/probe", "1.1.0", Some(true)),
                ORIGIN,
                DIGEST,
            )
            .expect("approved");
        assert_eq!(granted(&store, "local/probe"), ["log"]);
    }

    #[test]
    fn approve_from_another_origin_replaces_the_approval() {
        for_each_store(|store| {
            let optional = manifest("local/probe", "1.0.0", Some(false));
            store.approve(&optional, ORIGIN, DIGEST).expect("approved");
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("turned on");

            let approval = store
                .approve(&optional, OTHER_ORIGIN, DIGEST)
                .expect("approved from the other origin");
            assert_eq!(approval.replaced.as_deref(), Some(ORIGIN));
            let entry = entry(store, "local/probe").expect("approved");
            assert_eq!(entry.origin, OTHER_ORIGIN);
            assert!(
                entry.granted.is_empty(),
                "optional permissions start off again"
            );

            let approval = store
                .approve(&optional, OTHER_ORIGIN, DIGEST)
                .expect("approved again");
            assert_eq!(approval.replaced, None, "the same origin replaces nothing");
        });
    }

    #[test]
    fn set_optional_turns_one_permission_on_and_off() {
        for_each_store(|store| {
            let optional = manifest("local/probe", "1.0.0", Some(false));
            store.approve(&optional, ORIGIN, DIGEST).expect("approved");
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("on");
            assert_eq!(granted(store, "local/probe"), ["log"]);
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("on again");
            assert_eq!(granted(store, "local/probe"), ["log"]);
            store
                .set_optional(&optional, ORIGIN, "log", false)
                .expect("off");
            assert_eq!(granted(store, "local/probe"), Vec::<String>::new());
        });
    }

    #[test]
    fn set_optional_refuses_what_is_not_an_optional_permission_of_this_approval() {
        for_each_store(|store| {
            let required = manifest("local/probe", "1.0.0", Some(true));
            let optional = manifest("local/probe", "1.0.0", Some(false));
            let undeclared = manifest("local/probe", "1.0.0", None);
            let kind = |result: Result<(), RuntimeError>| result.map_err(|error| error.kind());

            assert_eq!(
                kind(store.set_optional(&optional, ORIGIN, "log", true)),
                Err(ErrorKind::NotGranted),
                "no approval"
            );

            store.approve(&required, ORIGIN, DIGEST).expect("approved");
            assert_eq!(
                kind(store.set_optional(&required, ORIGIN, "log", false)),
                Err(ErrorKind::RequiredTogether)
            );
            assert_eq!(
                kind(store.set_optional(&undeclared, ORIGIN, "log", true)),
                Err(ErrorKind::NotDeclared)
            );
            assert_eq!(
                kind(store.set_optional(&optional, ORIGIN, "telepathy", true)),
                Err(ErrorKind::UnknownPermission)
            );
            assert_eq!(
                kind(store.set_optional(&optional, OTHER_ORIGIN, "log", true)),
                Err(ErrorKind::NotGranted),
                "another origin"
            );
            assert_eq!(granted(store, "local/probe"), ["log"], "nothing changed");
        });
    }

    #[test]
    fn remove_deletes_the_approval() {
        for_each_store(|store| {
            store
                .approve(
                    &manifest("local/probe", "1.0.0", Some(true)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            store
                .approve(
                    &manifest("local/other", "1.0.0", Some(true)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            assert_eq!(store.remove(&id("local/probe")), Ok(true));
            assert_eq!(entry(store, "local/probe"), None);
            assert!(
                entry(store, "local/other").is_some(),
                "other plugins keep theirs"
            );
            assert_eq!(store.remove(&id("local/probe")), Ok(false));
        });
    }

    #[test]
    fn prune_removes_what_the_manifest_no_longer_declares() {
        for_each_store(|store| {
            store
                .approve(
                    &manifest("local/probe", "1.0.0", Some(true)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            let before = store.generation();
            assert_eq!(
                store.prune(&manifest("local/probe", "1.0.0", Some(true)), ORIGIN),
                Ok(Vec::new())
            );
            assert_eq!(
                store.generation(),
                before,
                "nothing to prune, nothing written"
            );

            // From another origin the approval is not this package's
            let quiet = manifest("local/probe", "1.1.0", None);
            assert_eq!(store.prune(&quiet, OTHER_ORIGIN), Ok(Vec::new()));
            assert_eq!(granted(store, "local/probe"), ["log"]);

            assert_eq!(store.prune(&quiet, ORIGIN), Ok(vec!["log".to_owned()]));
            assert_eq!(granted(store, "local/probe"), Vec::<String>::new());
            assert!(entry(store, "local/probe").is_some(), "still approved");
        });
    }

    #[test]
    fn status_shows_each_request_and_what_this_origin_is_granted() {
        for_each_store(|store| {
            let optional = manifest("local/probe", "1.0.0", Some(false));
            let status = |manifest: &Manifest, origin: &str| {
                store.status(manifest, origin).expect("readable")
            };
            let log = |required: bool, granted: bool| PermissionStatus {
                name: "log".to_owned(),
                required,
                granted,
            };

            assert_eq!(
                status(&optional, ORIGIN),
                Status {
                    permissions: vec![log(false, false)],
                    approval: None,
                    this_origin: false
                }
            );

            store.approve(&optional, ORIGIN, DIGEST).expect("approved");
            let approved = status(&optional, ORIGIN);
            assert!(approved.this_origin);
            assert_eq!(approved.permissions, [log(false, false)]);
            assert_eq!(
                approved.approval.map(|entry| entry.approved_version),
                Some("1.0.0".to_owned())
            );

            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("on");
            assert_eq!(status(&optional, ORIGIN).permissions, [log(false, true)]);

            // From another origin, nothing is granted, and the approval shows where it is from
            let elsewhere = status(&optional, OTHER_ORIGIN);
            assert!(!elsewhere.this_origin);
            assert_eq!(elsewhere.permissions, [log(false, false)]);
            assert_eq!(
                elsewhere.approval.map(|entry| entry.origin),
                Some(ORIGIN.to_owned())
            );

            // A version that requires log, still granted
            let required = manifest("local/probe", "1.1.0", Some(true));
            assert_eq!(status(&required, ORIGIN).permissions, [log(true, true)]);
        });
    }

    #[test]
    fn every_change_increments_the_generation() {
        for_each_store(|store| {
            let optional = manifest("local/probe", "1.0.0", Some(false));
            let mut last = store.generation();
            let mut changed = |store: &GrantStore| {
                let now = store.generation();
                assert!(now > last, "{now} after {last}");
                last = now;
            };
            store.approve(&optional, ORIGIN, DIGEST).expect("approved");
            changed(store);
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("on");
            changed(store);
            store
                .set_optional(&optional, ORIGIN, "log", false)
                .expect("off");
            changed(store);
            store
                .prune(&manifest("local/probe", "1.0.0", None), ORIGIN)
                .expect("pruned");
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("on");
            store
                .prune(&manifest("local/probe", "1.1.0", None), ORIGIN)
                .expect("pruned");
            changed(store);
            store.remove(&id("local/probe")).expect("removed");
            changed(store);

            let before = store.generation();
            entry(store, "local/probe");
            let _ = store.set_optional(&optional, ORIGIN, "log", true);
            assert_eq!(
                store.generation(),
                before,
                "reads and refusals change nothing"
            );
        });
    }

    #[test]
    fn two_threads_changing_one_file_lose_no_update() {
        let dir = TempDir::new().expect("a temporary directory");
        let path = Arc::new(dir.path().join("grants.json"));
        let threads: Vec<_> = (0..2)
            .map(|thread| {
                let path = Arc::clone(&path);
                thread::spawn(move || {
                    // Each thread has its own store, as two processes would
                    let store = GrantStore::open(path.as_path());
                    for n in 0..25 {
                        let plugin = manifest(&format!("local/t{thread}-{n}"), "1.0.0", Some(true));
                        store.approve(&plugin, ORIGIN, DIGEST).expect("approved");
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().expect("the thread finished");
        }
        let store = GrantStore::open(path.as_path());
        for thread in 0..2 {
            for n in 0..25 {
                assert!(
                    entry(&store, &format!("local/t{thread}-{n}")).is_some(),
                    "local/t{thread}-{n} was lost"
                );
            }
        }
    }

    /// A manifest for `local/github-releases` declaring `catalog/github` (required) with
    /// `list-releases`, and `local/weather` (optional) with `forecast`.
    fn connected_manifest(weather_operations: &[&str]) -> Manifest {
        let manifest = json!({
            "manifest_version": 1,
            "id": "local/github-releases",
            "version": "0.1.0",
            "author": "A",
            "permissions": [],
            "login_providers": [],
            "connectors": [
                { "id": "catalog/github", "major": 1, "operations": ["list-releases"], "required": true },
                { "id": "local/weather", "major": 0, "operations": weather_operations, "required": false }
            ]
        });
        Manifest::parse(&serde_json::to_vec(&manifest).expect("serializes")).expect("valid")
    }

    fn github_state() -> ConnectorState {
        ConnectorState {
            id: "catalog/github".to_owned(),
            origin: "catalog".to_owned(),
            version: "1.0.0".to_owned(),
            required: true,
            destinations: vec!["https://api.github.com".to_owned()],
            operations: BTreeMap::from([("list-releases".to_owned(), "sha256:51c0".to_owned())]),
            account: None,
        }
    }

    fn weather_state() -> ConnectorState {
        ConnectorState {
            id: "local/weather".to_owned(),
            origin: "/dev/connectors/weather.json".to_owned(),
            version: "0.1.0".to_owned(),
            required: false,
            destinations: vec!["http://127.0.0.1:8765".to_owned()],
            operations: BTreeMap::from([("forecast".to_owned(), "sha256:aa".to_owned())]),
            account: None,
        }
    }

    const CONNECTED: &str = "local/github-releases";

    fn approve_connected(store: &GrantStore, manifest: &Manifest, states: &[ConnectorState]) {
        let expected = approval_digest(DIGEST, manifest, states);
        store
            .approve_connected(manifest, ORIGIN, DIGEST, states, &expected)
            .expect("approved");
    }

    #[test]
    fn a_version_1_record_reads_as_version_4_without_connectors_and_is_written_back_as_4() {
        let (_dir, store, path) = store_with_text(&format!(
            r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log"],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
        ));
        let entry = entry(&store, "local/probe").expect("an entry");
        assert!(entry.connectors.is_empty());
        assert_eq!(entry.granted, ["log"]);
        store.remove(&id("local/other")).expect("no change");
        store
            .approve(&manifest("local/other", "1.0.0", None), ORIGIN, DIGEST)
            .expect("approved");
        let record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(record["record_version"], 4);
        assert_eq!(record["plugins"]["local/probe"]["granted"], json!(["log"]));
        // A version 1 record never has connectors
        let (_dir, store, _) = store_with_text(&format!(
            r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":[],"approved_version":"1.0.0","approved_digest":"{DIGEST}","connectors":{{"catalog/github":{{"origin":"catalog","version":"1.0.0","granted":true,"destinations":[],"operations":{{}}}}}}}}}}}}"#
        ));
        assert_eq!(
            store.entry(&id("local/probe")).unwrap_err().kind(),
            ErrorKind::NotGranted
        );
    }

    #[test]
    fn approve_connected_records_every_declared_connector() {
        for_each_store(|store| {
            let manifest = connected_manifest(&["forecast"]);
            approve_connected(store, &manifest, &[github_state(), weather_state()]);
            let entry = entry(store, CONNECTED).expect("an entry");
            assert_eq!(
                entry.connectors["catalog/github"],
                ConnectorGrant {
                    origin: "catalog".to_owned(),
                    version: "1.0.0".to_owned(),
                    granted: true,
                    destinations: vec!["https://api.github.com".to_owned()],
                    operations: BTreeMap::from([(
                        "list-releases".to_owned(),
                        "sha256:51c0".to_owned()
                    )]),
                    account: None,
                }
            );
            // Optional connectors start off
            assert!(!entry.connectors["local/weather"].granted);
            assert!(entry.is_connector_granted("catalog/github"));
            assert!(!entry.is_connector_granted("local/weather"));
            assert!(!entry.is_connector_granted("catalog/missing"));
        });
    }

    #[test]
    fn the_file_format_of_record_version_2() {
        let (_dir, store) = file_store();
        let manifest = connected_manifest(&["forecast"]);
        approve_connected(&store, &manifest, &[github_state(), weather_state()]);
        let record: Value =
            serde_json::from_slice(&fs::read(store.path().unwrap()).unwrap()).unwrap();
        assert_eq!(
            record["plugins"][CONNECTED]["connectors"]["catalog/github"],
            json!({
                "origin": "catalog",
                "version": "1.0.0",
                "granted": true,
                "destinations": ["https://api.github.com"],
                "operations": { "list-releases": "sha256:51c0" }
            })
        );
    }

    #[test]
    fn approve_connected_refuses_a_digest_of_anything_else_and_grants_nothing() {
        for_each_store(|store| {
            let manifest = connected_manifest(&["forecast"]);
            let shown = approval_digest(DIGEST, &manifest, &[github_state(), weather_state()]);
            let mut changed = weather_state();
            changed
                .destinations
                .push("https://api.weather.test".to_owned());
            let error = store
                .approve_connected(
                    &manifest,
                    ORIGIN,
                    DIGEST,
                    &[github_state(), changed],
                    &shown,
                )
                .unwrap_err();
            assert_eq!(error.kind(), ErrorKind::ApprovalChanged, "{error}");
            assert!(entry(store, CONNECTED).is_none());
            let error = store
                .approve_connected(
                    &manifest,
                    ORIGIN,
                    DIGEST,
                    &[github_state(), weather_state()],
                    "sha256:00",
                )
                .unwrap_err();
            assert_eq!(error.kind(), ErrorKind::ApprovalChanged);
            assert_eq!(store.generation(), 0);
        });
    }

    #[test]
    fn the_approval_digest_covers_everything_an_approval_records() {
        let manifest = connected_manifest(&["forecast"]);
        let states = [github_state(), weather_state()];
        let base = approval_digest(DIGEST, &manifest, &states);
        assert!(base.starts_with("sha256:"));
        assert_eq!(base.len(), "sha256:".len() + 64);
        // Stable, and independent of the order the connectors come in
        assert_eq!(base, approval_digest(DIGEST, &manifest, &states));
        assert_eq!(
            base,
            approval_digest(DIGEST, &manifest, &[weather_state(), github_state()])
        );
        let edits: [fn(&mut ConnectorState); 6] = [
            |state| state.id = "local/other".to_owned(),
            |state| state.origin = "/elsewhere/weather.json".to_owned(),
            |state| state.version = "0.2.0".to_owned(),
            |state| state.required = true,
            |state| state.destinations.push("https://x.test".to_owned()),
            |state| {
                state
                    .operations
                    .insert("forecast".to_owned(), "sha256:bb".to_owned());
            },
        ];
        for edit in edits {
            let mut weather = weather_state();
            edit(&mut weather);
            assert_ne!(
                base,
                approval_digest(DIGEST, &manifest, &[github_state(), weather])
            );
        }
        assert_ne!(base, approval_digest("sha256:11", &manifest, &states));
        let with_log = {
            let mut value: Value = serde_json::from_str(r#"{
                "manifest_version": 1, "id": "local/github-releases", "version": "0.1.0", "author": "A",
                "permissions": [{ "name": "log", "required": false }], "login_providers": [],
                "connectors": [
                    { "id": "catalog/github", "major": 1, "operations": ["list-releases"], "required": true },
                    { "id": "local/weather", "major": 0, "operations": ["forecast"], "required": false }
                ]
            }"#).unwrap();
            let bytes = serde_json::to_vec(&value).unwrap();
            value.take();
            Manifest::parse(&bytes).unwrap()
        };
        assert_ne!(base, approval_digest(DIGEST, &with_log, &states));
    }

    #[test]
    fn an_optional_connector_keeps_its_state_when_nothing_changed_and_is_off_otherwise() {
        for_each_store(|store| {
            let manifest = connected_manifest(&["forecast"]);
            approve_connected(store, &manifest, &[github_state(), weather_state()]);
            store
                .set_connector(&manifest, ORIGIN, "local/weather", true)
                .expect("turned on");
            approve_connected(store, &manifest, &[github_state(), weather_state()]);
            assert!(
                entry(store, CONNECTED)
                    .unwrap()
                    .is_connector_granted("local/weather")
            );
            let mut changed = weather_state();
            changed
                .operations
                .insert("forecast".to_owned(), "sha256:bb".to_owned());
            approve_connected(store, &manifest, &[github_state(), changed]);
            assert!(
                !entry(store, CONNECTED)
                    .unwrap()
                    .is_connector_granted("local/weather")
            );
        });
    }

    #[test]
    fn set_connector_turns_an_optional_connector_on_and_off() {
        for_each_store(|store| {
            let manifest = connected_manifest(&["forecast"]);
            approve_connected(store, &manifest, &[github_state(), weather_state()]);
            let before = store.generation();
            store
                .set_connector(&manifest, ORIGIN, "local/weather", true)
                .unwrap();
            assert!(
                store
                    .connector_granted(&id(CONNECTED), ORIGIN, "local/weather")
                    .unwrap()
            );
            assert_eq!(store.generation(), before + 1);
            store
                .set_connector(&manifest, ORIGIN, "local/weather", true)
                .unwrap();
            assert_eq!(
                store.generation(),
                before + 1,
                "no change, no new generation"
            );
            store
                .set_connector(&manifest, ORIGIN, "local/weather", false)
                .unwrap();
            assert!(
                !store
                    .connector_granted(&id(CONNECTED), ORIGIN, "local/weather")
                    .unwrap()
            );
            assert!(
                !store
                    .connector_granted(&id(CONNECTED), OTHER_ORIGIN, "catalog/github")
                    .unwrap()
            );

            let kind = |result: Result<(), RuntimeError>| result.unwrap_err().kind();
            assert_eq!(
                kind(store.set_connector(&manifest, ORIGIN, "catalog/github", false)),
                ErrorKind::RequiredTogether
            );
            assert_eq!(
                kind(store.set_connector(&manifest, ORIGIN, "local/other", true)),
                ErrorKind::NotDeclared
            );
            assert_eq!(
                kind(store.set_connector(&manifest, OTHER_ORIGIN, "local/weather", true)),
                ErrorKind::NotGranted
            );
        });
    }

    #[test]
    fn prune_removes_connectors_the_manifest_no_longer_declares() {
        for_each_store(|store| {
            let manifest = connected_manifest(&["forecast"]);
            approve_connected(store, &manifest, &[github_state(), weather_state()]);
            let without_weather = Manifest::parse(
                &serde_json::to_vec(&json!({
                    "manifest_version": 1, "id": CONNECTED, "version": "0.2.0", "author": "A",
                    "permissions": [], "login_providers": [],
                    "connectors": [{ "id": "catalog/github", "major": 1, "operations": ["list-releases"], "required": true }]
                }))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(
                store.prune(&without_weather, ORIGIN).unwrap(),
                ["local/weather"]
            );
            let entry = entry(store, CONNECTED).unwrap();
            assert!(entry.connectors.contains_key("catalog/github"));
            assert!(!entry.connectors.contains_key("local/weather"));
        });
    }

    #[test]
    fn connector_changes_name_what_needs_a_new_approval() {
        let store = GrantStore::in_memory();
        let manifest = connected_manifest(&["forecast"]);
        approve_connected(&store, &manifest, &[github_state(), weather_state()]);
        store
            .set_connector(&manifest, ORIGIN, "local/weather", true)
            .unwrap();
        let entry = entry(&store, CONNECTED).unwrap();
        assert!(
            entry
                .connector_changes(&manifest, &[github_state(), weather_state()])
                .is_empty()
        );

        // A new version that only adds operations the widget does not declare changes nothing
        let mut github = github_state();
        github.version = "1.1.0".to_owned();
        assert!(
            entry
                .connector_changes(&manifest, &[github, weather_state()])
                .is_empty()
        );

        let mut github = github_state();
        github
            .destinations
            .push("https://uploads.github.com".to_owned());
        assert_eq!(
            entry.connector_changes(&manifest, &[github, weather_state()]),
            ["destination added: https://uploads.github.com"]
        );
        let mut github = github_state();
        github
            .operations
            .insert("list-releases".to_owned(), "sha256:99".to_owned());
        assert_eq!(
            entry.connector_changes(&manifest, &[github, weather_state()]),
            ["operation changed: list-releases"]
        );
        let mut weather = weather_state();
        weather.origin = "/elsewhere/weather.json".to_owned();
        assert_eq!(
            entry
                .connector_changes(&manifest, &[github_state(), weather])
                .len(),
            1
        );
        let more = connected_manifest(&["forecast", "now"]);
        let mut weather = weather_state();
        weather
            .operations
            .insert("now".to_owned(), "sha256:cc".to_owned());
        assert_eq!(
            entry.connector_changes(&more, &[github_state(), weather]),
            ["operation newly declared: now"]
        );
        // An optional connector that is off needs nothing, whatever changed
        store
            .set_connector(&manifest, ORIGIN, "local/weather", false)
            .unwrap();
        let entry = self::entry(&store, CONNECTED).unwrap();
        let mut weather = weather_state();
        weather.destinations.push("https://x.test".to_owned());
        assert!(
            entry
                .connector_changes(&manifest, &[github_state(), weather])
                .is_empty()
        );
    }

    #[test]
    fn a_new_connector_version_that_changes_nothing_is_recorded_on_load() {
        let store = GrantStore::in_memory();
        let manifest = connected_manifest(&["forecast"]);
        approve_connected(&store, &manifest, &[github_state(), weather_state()]);
        let mut github = github_state();
        github.version = "1.2.0".to_owned();
        store
            .refresh_connector_versions(&manifest, ORIGIN, &[github, weather_state()])
            .unwrap();
        assert_eq!(
            entry(&store, CONNECTED).unwrap().connectors["catalog/github"].version,
            "1.2.0"
        );
        let generation = store.generation();
        store
            .refresh_connector_versions(&manifest, ORIGIN, &[github_state(), weather_state()])
            .unwrap();
        assert_eq!(store.generation(), generation + 1);
        store
            .refresh_connector_versions(&manifest, ORIGIN, &[github_state(), weather_state()])
            .unwrap();
        assert_eq!(
            store.generation(),
            generation + 1,
            "nothing moved, nothing written"
        );
    }

    #[test]
    fn a_change_from_another_store_on_the_same_file_is_seen_at_once() {
        let (_dir, store) = file_store();
        let manifest = connected_manifest(&["forecast"]);
        approve_connected(&store, &manifest, &[github_state(), weather_state()]);
        let other = GrantStore::open(store.path().unwrap());
        other
            .set_connector(&manifest, ORIGIN, "local/weather", true)
            .unwrap();
        assert!(
            store
                .connector_granted(&id(CONNECTED), ORIGIN, "local/weather")
                .unwrap()
        );
        other.remove(&id(CONNECTED)).unwrap();
        assert!(
            !store
                .connector_granted(&id(CONNECTED), ORIGIN, "catalog/github")
                .unwrap()
        );
    }

    // Feature 004: grant record version 3 and storage (`specs/004-plugin-storage/data-model.md`)

    /// A manifest for `local/update-counter` declaring storage with this quota.
    fn stored_manifest(required: bool, quota_bytes: u64) -> Manifest {
        let manifest = json!({
            "manifest_version": 1,
            "id": STORED,
            "version": "0.1.0",
            "author": "A",
            "permissions": [{ "name": "storage", "required": required, "quota_bytes": quota_bytes }],
            "login_providers": []
        });
        Manifest::parse(&serde_json::to_vec(&manifest).expect("serializes")).expect("valid")
    }

    const STORED: &str = "local/update-counter";

    fn approve_stored(store: &GrantStore, manifest: &Manifest) {
        let expected = approval_digest(DIGEST, manifest, &[]);
        store
            .approve_connected(manifest, ORIGIN, DIGEST, &[], &expected)
            .expect("approved");
    }

    #[test]
    fn a_version_2_record_reads_as_version_4_and_is_written_back_as_4() {
        let (_dir, store, path) = store_with_text(&format!(
            r#"{{"record_version":2,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log"],"approved_version":"1.0.0","approved_digest":"{DIGEST}","connectors":{{}}}}}}}}"#
        ));
        let entry = entry(&store, "local/probe").expect("an entry");
        assert_eq!(entry.storage_quota_bytes, None);
        store
            .approve(&manifest("local/other", "1.0.0", None), ORIGIN, DIGEST)
            .expect("approved");
        let record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(record["record_version"], 4);
        assert!(
            record["plugins"]["local/probe"]
                .get("storage_quota_bytes")
                .is_none()
        );
    }

    /// A connector with a credential (`specs/005-plugin-credentials/data-model.md`, Grant record,
    /// version 4).
    fn notes_state(uses: bool) -> ConnectorState {
        ConnectorState {
            id: "local/notes".to_owned(),
            origin: "/dev/connectors/notes.json".to_owned(),
            version: "1.0.0".to_owned(),
            required: true,
            destinations: vec!["https://notes.test".to_owned()],
            operations: BTreeMap::from([("list-notes".to_owned(), "sha256:cc".to_owned())]),
            account: Some(AccountState {
                uses,
                required: false,
                credential: "sha256:4d1a".to_owned(),
                access: vec![Access {
                    name: "notes".to_owned(),
                    level: crate::connector::Level::Read,
                }],
            }),
        }
    }

    fn notes_manifest(account: bool) -> Manifest {
        let manifest = json!({
            "manifest_version": 1, "id": "local/private-notes", "version": "0.1.0", "author": "A",
            "permissions": [],
            "connectors": [{ "id": "local/notes", "major": 1, "operations": ["list-notes"],
                             "required": true, "account": account }]
        });
        Manifest::parse(&serde_json::to_vec(&manifest).unwrap()).unwrap()
    }

    #[test]
    fn an_approval_records_the_account_of_a_connector_with_a_credential() {
        for_each_store(|store| {
            let manifest = notes_manifest(true);
            approve_connected(store, &manifest, &[notes_state(true)]);
            let entry = entry(store, "local/private-notes").unwrap();
            assert_eq!(
                entry.connectors["local/notes"].account,
                Some(AccountGrant {
                    uses: true,
                    credential: "sha256:4d1a".to_owned(),
                    access: vec![Access {
                        name: "notes".to_owned(),
                        level: crate::connector::Level::Read,
                    }],
                })
            );
            assert!(entry.uses_account("local/notes"));
            assert!(!entry.uses_account("catalog/github"));
        });
        // In the file, `account` appears only for a connector with a credential
        let (_dir, store) = file_store();
        approve_connected(&store, &notes_manifest(false), &[notes_state(false)]);
        approve_connected(&store, &connected_manifest(&["forecast"]), &[github_state(), weather_state()]);
        let record: Value = serde_json::from_slice(&fs::read(store.path().unwrap()).unwrap()).unwrap();
        assert_eq!(record["record_version"], 4);
        assert_eq!(
            record["plugins"]["local/private-notes"]["connectors"]["local/notes"]["account"],
            json!({ "uses": false, "credential": "sha256:4d1a", "access": [{ "name": "notes", "level": "read" }] })
        );
        assert!(record["plugins"][CONNECTED]["connectors"]["catalog/github"].get("account").is_none());
    }

    #[test]
    fn a_version_3_record_reads_as_version_4_and_an_older_one_never_holds_an_account() {
        let (_dir, store, path) = store_with_text(&format!(
            r#"{{"record_version":3,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log"],"approved_version":"1.0.0","approved_digest":"{DIGEST}","connectors":{{"catalog/github":{{"origin":"catalog","version":"1.0.0","granted":true,"destinations":[],"operations":{{}}}}}}}}}}}}"#
        ));
        assert_eq!(entry(&store, "local/probe").unwrap().connectors["catalog/github"].account, None);
        store
            .approve(&manifest("local/other", "1.0.0", None), ORIGIN, DIGEST)
            .expect("approved");
        let record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(record["record_version"], 4);
        for version in [1, 2, 3] {
            let (_dir, store, _) = store_with_text(&format!(
                r#"{{"record_version":{version},"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":[],"approved_version":"1.0.0","approved_digest":"{DIGEST}","connectors":{{"local/notes":{{"origin":"/n.json","version":"1.0.0","granted":true,"destinations":[],"operations":{{}},"account":{{"uses":true,"credential":"sha256:1","access":[]}}}}}}}}}}}}"#
            ));
            assert_eq!(
                store.entry(&id("local/probe")).unwrap_err().kind(),
                ErrorKind::NotGranted,
                "version {version}"
            );
        }
    }

    #[test]
    fn the_approval_digest_covers_the_account_of_a_connector_with_a_credential() {
        let manifest = notes_manifest(true);
        let base = approval_digest(DIGEST, &manifest, &[notes_state(true)]);
        let edits: [fn(&mut AccountState); 3] = [
            |account| account.uses = false,
            |account| account.credential = "sha256:ff".to_owned(),
            |account| account.access[0].level = crate::connector::Level::Write,
        ];
        for edit in edits {
            let mut state = notes_state(true);
            edit(state.account.as_mut().unwrap());
            assert_ne!(base, approval_digest(DIGEST, &manifest, &[state]));
        }
        let mut without = notes_state(true);
        without.account = None;
        assert_ne!(base, approval_digest(DIGEST, &manifest, &[without]));
    }

    /// Research R8: what changes in a credential needs a new approval, and what does not.
    #[test]
    fn credential_changes_follow_the_table_of_research_r8() {
        let store = GrantStore::in_memory();
        approve_connected(&store, &notes_manifest(true), &[notes_state(true)]);
        let approved = entry(&store, "local/private-notes").unwrap();
        let changes = |edit: fn(&mut ConnectorState), account: bool| {
            let mut state = notes_state(account);
            edit(&mut state);
            approved.connector_changes(&notes_manifest(account), &[state])
        };
        assert!(changes(|_| {}, true).is_empty());
        let change = changes(|s| s.account.as_mut().unwrap().credential = "sha256:ff".to_owned(), true);
        assert_eq!(change, ["credential changed: local/notes"]);
        let change = changes(
            |s| {
                s.account.as_mut().unwrap().access.push(Access {
                    name: "drafts".to_owned(),
                    level: crate::connector::Level::Read,
                })
            },
            true,
        );
        assert_eq!(change, ["access added: local/notes drafts read"]);
        let change = changes(|s| s.account.as_mut().unwrap().access[0].level = crate::connector::Level::Write, true);
        assert_eq!(change, ["access added: local/notes notes write"]);
        // Narrowed access, or no longer using the account: nothing to approve
        assert!(changes(|s| s.account.as_mut().unwrap().access.clear(), true).is_empty());
        assert!(changes(|_| {}, false).is_empty());

        // A credential added to a connector the widget uses, or starting to use the account
        let store = GrantStore::in_memory();
        let mut plain = notes_state(false);
        plain.account = None;
        approve_connected(&store, &notes_manifest(false), &[plain]);
        let approved = entry(&store, "local/private-notes").unwrap();
        assert_eq!(
            approved.connector_changes(&notes_manifest(true), &[notes_state(true)]),
            ["credential added: local/notes"]
        );
        let store = GrantStore::in_memory();
        approve_connected(&store, &notes_manifest(false), &[notes_state(false)]);
        let approved = entry(&store, "local/private-notes").unwrap();
        assert_eq!(
            approved.connector_changes(&notes_manifest(true), &[notes_state(true)]),
            ["account use newly declared: local/notes"]
        );
        // A credential the widget neither uses nor needs changes nothing
        let mut unused = notes_state(false);
        unused.account.as_mut().unwrap().credential = "sha256:ff".to_owned();
        assert!(approved.connector_changes(&notes_manifest(false), &[unused]).is_empty());
    }

    #[test]
    fn only_a_version_3_record_holds_a_storage_quota() {
        for version in [1, 2] {
            let (_dir, store, _) = store_with_text(&format!(
                r#"{{"record_version":{version},"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":[],"approved_version":"1.0.0","approved_digest":"{DIGEST}","storage_quota_bytes":4096}}}}}}"#
            ));
            assert_eq!(
                store.entry(&id("local/probe")).unwrap_err().kind(),
                ErrorKind::NotGranted,
                "version {version}"
            );
        }
        let (_dir, store, _) = store_with_text(&format!(
            r#"{{"record_version":3,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["storage"],"approved_version":"1.0.0","approved_digest":"{DIGEST}","connectors":{{}},"storage_quota_bytes":4096}}}}}}"#
        ));
        assert_eq!(
            entry(&store, "local/probe").unwrap().storage_quota_bytes,
            Some(4096)
        );
    }

    #[test]
    fn an_approval_records_the_quota_it_showed() {
        for_each_store(|store| {
            approve_stored(store, &stored_manifest(true, 65536));
            let approved = entry(store, STORED).expect("an entry");
            assert_eq!(approved.storage_quota_bytes, Some(65536));
            assert_eq!(approved.granted, ["storage"]);
            // Smaller and larger alike: the approval keeps the quota it last showed
            approve_stored(store, &stored_manifest(true, 4096));
            assert_eq!(
                entry(store, STORED).unwrap().storage_quota_bytes,
                Some(4096)
            );
            approve_stored(store, &stored_manifest(true, 131072));
            assert_eq!(
                entry(store, STORED).unwrap().storage_quota_bytes,
                Some(131072)
            );
        });
        let (_dir, store) = file_store();
        approve_stored(&store, &stored_manifest(false, 4096));
        let record: Value =
            serde_json::from_slice(&fs::read(store.path().unwrap()).unwrap()).unwrap();
        assert_eq!(record["plugins"][STORED]["storage_quota_bytes"], 4096);
        assert_eq!(record["plugins"][STORED]["granted"], json!([]));
    }

    #[test]
    fn a_plugin_with_storage_is_approved_only_with_its_digest() {
        for_each_store(|store| {
            for required in [true, false] {
                let manifest = stored_manifest(required, 65536);
                let error = store.approve(&manifest, ORIGIN, DIGEST).unwrap_err();
                assert_eq!(error.kind(), ErrorKind::ApprovalChanged, "{error}");
                let shown = approval_digest(DIGEST, &stored_manifest(required, 4096), &[]);
                let error = store
                    .approve_connected(&manifest, ORIGIN, DIGEST, &[], &shown)
                    .unwrap_err();
                assert_eq!(error.kind(), ErrorKind::ApprovalChanged, "{error}");
                assert!(entry(store, STORED).is_none());
                assert_eq!(store.generation(), 0);
            }
        });
    }

    #[test]
    fn the_approval_digest_without_storage_is_that_of_feature_003() {
        // Computed with feature 003's code, before storage existed: widgets that declare no
        // storage keep their digest, and so their kept data
        assert_eq!(
            approval_digest(
                DIGEST,
                &connected_manifest(&["forecast"]),
                &[github_state(), weather_state()]
            ),
            "sha256:4f360db672279a9cded20d52055377ad8e2130dbf461f26765312ce83d74c930"
        );
        assert_eq!(
            approval_digest(DIGEST, &manifest("local/probe", "1.0.0", Some(true)), &[]),
            "sha256:c4941f3d75a062a58ab369ba0479a284ef5eca1e90abff96a4e6870782a3f139"
        );
    }

    #[test]
    fn the_approval_digest_covers_the_storage_declaration() {
        let base = approval_digest(DIGEST, &stored_manifest(true, 65536), &[]);
        assert_eq!(
            base,
            approval_digest(DIGEST, &stored_manifest(true, 65536), &[])
        );
        assert_ne!(
            base,
            approval_digest(DIGEST, &stored_manifest(false, 65536), &[])
        );
        assert_ne!(
            base,
            approval_digest(DIGEST, &stored_manifest(true, 65537), &[])
        );
        let without = {
            let manifest = json!({
                "manifest_version": 1, "id": STORED, "version": "0.1.0", "author": "A",
                "permissions": [{ "name": "storage", "required": true, "quota_bytes": 65536 }],
                "login_providers": []
            });
            let mut text = serde_json::to_string(&manifest).unwrap();
            text = text.replace(r#","quota_bytes":65536"#, "");
            text = text.replace(r#""name":"storage""#, r#""name":"log""#);
            Manifest::parse(text.as_bytes()).unwrap()
        };
        assert_ne!(base, approval_digest(DIGEST, &without, &[]));
    }

    #[test]
    fn a_larger_quota_than_approved_needs_a_new_approval() {
        let store = GrantStore::in_memory();
        approve_stored(&store, &stored_manifest(true, 65536));
        let approved = entry(&store, STORED).unwrap();
        assert_eq!(approved.storage_change(&stored_manifest(true, 65536)), None);
        assert_eq!(approved.storage_change(&stored_manifest(true, 4096)), None);
        let change = approved
            .storage_change(&stored_manifest(true, 131072))
            .expect("a larger quota");
        assert!(
            change.contains("65536") && change.contains("131072"),
            "{change}"
        );
        // Never approved: the declared quota needs approval
        let mut never = approved.clone();
        never.storage_quota_bytes = None;
        let change = never
            .storage_change(&stored_manifest(true, 4096))
            .expect("not approved yet");
        assert!(change.contains("4096"), "{change}");
        // Optional storage that is off needs nothing; turned on, it does. A fresh record: an
        // approval from the same origin keeps what is still declared and on
        let store = GrantStore::in_memory();
        approve_stored(&store, &stored_manifest(false, 4096));
        let off = entry(&store, STORED).unwrap();
        assert!(!off.is_granted("storage"));
        assert_eq!(off.storage_change(&stored_manifest(false, 131072)), None);
        let mut on = off.clone();
        on.granted = vec!["storage".to_owned()];
        assert!(on.storage_change(&stored_manifest(false, 131072)).is_some());
        // A manifest without storage
        assert_eq!(
            approved.storage_change(&manifest(STORED, "0.1.0", None)),
            None
        );
    }

    #[test]
    fn turning_storage_on_checks_the_approved_quota() {
        for_each_store(|store| {
            approve_stored(store, &stored_manifest(false, 4096));
            let error = store
                .set_optional(&stored_manifest(false, 8192), ORIGIN, "storage", true)
                .unwrap_err();
            assert_eq!(error.kind(), ErrorKind::NeedsNewApproval, "{error}");
            assert!(
                error.to_string().contains("4096") && error.to_string().contains("8192"),
                "{error}"
            );
            assert!(!entry(store, STORED).unwrap().is_granted("storage"));
            store
                .set_optional(&stored_manifest(false, 2048), ORIGIN, "storage", true)
                .expect("within the approved quota");
            assert!(entry(store, STORED).unwrap().is_granted("storage"));
        });
    }

    #[test]
    fn a_granted_permission_is_read_from_the_record_as_it_is_now() {
        let (_dir, store) = file_store();
        approve_stored(&store, &stored_manifest(false, 4096));
        let plugin = id(STORED);
        assert!(
            !store
                .permission_granted(&plugin, ORIGIN, "storage")
                .unwrap()
        );
        // Another process's change, through another store on the same file
        let other = GrantStore::open(store.path().unwrap());
        other
            .set_optional(&stored_manifest(false, 4096), ORIGIN, "storage", true)
            .unwrap();
        assert!(
            store
                .permission_granted(&plugin, ORIGIN, "storage")
                .unwrap()
        );
        assert!(
            !store
                .permission_granted(&plugin, OTHER_ORIGIN, "storage")
                .unwrap()
        );
    }

    #[test]
    fn while_granted_runs_only_while_the_permission_is_granted() {
        for_each_store(|store| {
            let plugin = id(STORED);
            assert_eq!(
                store
                    .while_granted(&plugin, ORIGIN, "storage", || 1)
                    .unwrap(),
                None
            );
            approve_stored(store, &stored_manifest(true, 4096));
            assert_eq!(
                store
                    .while_granted(&plugin, ORIGIN, "storage", || 2)
                    .unwrap(),
                Some(2)
            );
            assert_eq!(
                store
                    .while_granted(&plugin, OTHER_ORIGIN, "storage", || 3)
                    .unwrap(),
                None
            );
            assert_eq!(
                store.while_granted(&plugin, ORIGIN, "log", || 4).unwrap(),
                None
            );
        });
    }

    #[test]
    fn a_grant_change_waits_until_while_granted_returns() {
        use std::sync::atomic::AtomicBool;
        use std::thread;
        use std::time::{Duration, Instant};

        let (_dir, store) = file_store();
        approve_stored(&store, &stored_manifest(true, 4096));
        let path = store.path().unwrap().to_owned();
        let committing = AtomicBool::new(false);
        let committed_at = Mutex::new(None);
        thread::scope(|scope| {
            let remover = scope.spawn(|| {
                let started = Instant::now();
                while !committing.load(Ordering::Acquire) {
                    // A test fails, never hangs, when the commit never runs
                    assert!(
                        started.elapsed() < Duration::from_secs(10),
                        "the commit never ran"
                    );
                    thread::yield_now();
                }
                // Another process removing the approval while the commit runs
                GrantStore::open(&path).remove(&id(STORED)).unwrap();
                Instant::now()
            });
            let ran = store
                .while_granted(&id(STORED), ORIGIN, "storage", || {
                    committing.store(true, Ordering::Release);
                    thread::sleep(Duration::from_millis(300));
                    *committed_at.lock().unwrap() = Some(Instant::now());
                })
                .unwrap();
            assert_eq!(ran, Some(()));
            let removed_at = remover.join().unwrap();
            let committed_at = committed_at.lock().unwrap().unwrap();
            assert!(
                removed_at >= committed_at,
                "the removal did not wait for the commit"
            );
        });
        assert_eq!(entry(&store, STORED), None);
    }
}
