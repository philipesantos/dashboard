//! Plugin runtime: loads plugins packaged as WebAssembly components and runs them in a sandbox,
//! under limits set by the host, with exactly the capabilities their manifest declares and the
//! user granted (`specs/002-plugin-permissions/`), and reach external services only through
//! the connectors they declare, which the host runs (`specs/003-plugin-connectors/`), and keep
//! data only in a store of their own (`specs/004-plugin-storage/`).

// The test seam replaces DNS and the trusted roots; it must never reach a build anyone runs
// (specs/003-plugin-connectors/research.md, R15).
#[cfg(all(feature = "test-network", not(debug_assertions)))]
compile_error!(
    "the test-network feature is only for the runtime's own tests, which run in debug builds: a release build never includes the test seam"
);

pub mod accounts;
mod calls;
mod catalog;
pub mod connector;
mod error;
pub mod grants;
mod host;
mod kept;
mod limits;
pub mod manifest;
mod network;
pub mod package;
mod permissions;
mod records;
pub mod requests;
mod signin;
pub mod storage;
mod text;
mod vault;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use wasmtime::component::{Component, ComponentExportIndex, InstancePre, Linker};
use wasmtime::{Config, Engine, Store, Trap};

use calls::{Calls, ConnectorTable, Phase, Rates, RunLog};
use connector::{Connector, Source};
use grants::{AccountState, Approval, ConnectorState, GrantStore};
use host::HostState;
use kept::{Found, KeptStore};
use limits::MemoryLimitExceeded;
use manifest::Manifest;
use permissions::Interfaces;
use requests::RequestLog;
use storage::StorageRun;

pub use calls::CallCounts;
pub use error::{ErrorKind, RuntimeError};
pub use limits::Limits;
#[cfg(feature = "test-network")]
pub use network::{Route, TestNetwork};
pub use permissions::{Contract, ContractVersion};
pub use storage::StorageCounts;
pub use text::{escape, quote_entry};

/// Bindings for the `widget` world of plugin contract v0 (`wit/v0`, `dashboard:widget@0.1.0`).
pub(crate) mod v0 {
    wasmtime::component::bindgen!({ path: "../wit/v0", world: "widget" });
}

/// Bindings for plugin contract v1 (`wit/v1`, `dashboard:widget@1.3.0`), from its widest world,
/// `account-stored-widget`, for the host side of every interface. Wasmtime matches the imports
/// of plugins built for an older minor against them by compatible version. Exports are looked up
/// one by one, by the exact names each plugin has, since a render-only widget has no `updater`
/// (`specs/003-plugin-connectors/research.md`, R11; `specs/004-plugin-storage/research.md`, R13).
pub(crate) mod v1 {
    wasmtime::component::bindgen!({ path: "../wit/v1", world: "account-stored-widget" });
}

/// The first 8 bytes of every binary WebAssembly component: the magic number, then the
/// component version and layer.
const COMPONENT_PREAMBLE: [u8; 8] = [0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00];

/// Severity of a plugin log message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// The level's name, as the CLI prints it.
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

/// A message a plugin wrote through the `log` capability during one render call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogMessage {
    pub level: LogLevel,
    pub message: String,
}

/// The outcome of one render call.
#[derive(Debug)]
pub struct RenderReport {
    /// The widget content, or why the call was refused or stopped. On error there is no
    /// content, even if the plugin produced some before it stopped.
    pub result: Result<serde_json::Value, RuntimeError>,
    /// Log messages written before the call ended, in order.
    pub logs: Vec<LogMessage>,
    /// Messages over the per-call cap, dropped instead of kept.
    pub dropped_logs: u32,
    /// What the render did with storage, for a plugin granted it.
    pub storage: Option<StorageCounts>,
}

/// The outcome of one update call (`specs/003-plugin-connectors/`, FR-010).
#[derive(Debug)]
pub struct UpdateReport {
    /// The data the update returned, now kept, or why the call was refused, stopped or failed,
    /// in which case the previous data is kept.
    pub result: Result<serde_json::Value, RuntimeError>,
    pub logs: Vec<LogMessage>,
    pub dropped_logs: u32,
    /// How the update's connector calls went.
    pub calls: CallCounts,
    /// What the update did with storage, for a plugin granted it, when its writes were kept or
    /// it failed for another reason.
    pub storage: Option<StorageCounts>,
}

/// What the host decides when it starts the runtime: where its data lives, and whether it runs
/// in development mode, the only mode in which local connectors load (FR-018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeOptions {
    development: bool,
    connectors_dir: Option<PathBuf>,
    data_dir: PathBuf,
}

impl RuntimeOptions {
    /// Options with `data_dir` for kept data and the request log, development mode off and
    /// local connectors read from `<data_dir>/connectors/` once it is on.
    pub fn new(data_dir: impl Into<PathBuf>) -> RuntimeOptions {
        RuntimeOptions {
            development: false,
            connectors_dir: None,
            data_dir: data_dir.into(),
        }
    }

    /// Turns development mode on or off. Only the development command-line tool turns it on;
    /// the host app of Phase 3 never does.
    pub fn development(mut self, on: bool) -> RuntimeOptions {
        self.development = on;
        self
    }

    /// Reads local connectors from `dir`, `<dir>/<name>.json` for `local/<name>`.
    pub fn connectors_dir(mut self, dir: impl Into<PathBuf>) -> RuntimeOptions {
        self.connectors_dir = Some(dir.into());
        self
    }

    pub fn is_development(&self) -> bool {
        self.development
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Where local connectors are read from.
    pub fn local_connectors_dir(&self) -> PathBuf {
        self.connectors_dir
            .clone()
            .unwrap_or_else(|| self.data_dir.join("connectors"))
    }

    /// `<data dir>/dev.philsantos.dashboard/development/`, beside the development grant record,
    /// or `None` when the system has no data directory.
    pub fn default_development_data_dir() -> Option<PathBuf> {
        dirs::data_dir().map(|dir| dir.join(grants::APP_DIR).join("development"))
    }
}

/// Loads plugins and runs them under the host's limits.
pub struct Runtime {
    engine: Engine,
    limits: Limits,
    clock: Arc<EpochClock>,
    options: RuntimeOptions,
    catalog: catalog::Catalog,
    network: network::Network,
    /// Calls per minute per widget and per destination host, across every plugin this runtime
    /// runs (research R10).
    rates: Arc<Rates>,
    /// The secrets vault, one ordered thread for every plugin this runtime runs
    /// (`specs/005-plugin-credentials/research.md`, R5).
    vault: Arc<vault::Vault>,
}

/// What disconnecting an account did (FR-014).
#[derive(Debug)]
pub struct Disconnected {
    pub account: accounts::Account,
    /// Whether the service revoked the token, when the connector says where; `None` when it
    /// offers no revocation, or for a key.
    pub revoked: Option<Result<(), String>>,
    /// Where the person revokes it by hand, when the connector names a page.
    pub revoke_page: Option<String>,
    /// The widgets whose kept data, read as that account, was deleted.
    pub kept_discarded: Vec<String>,
}

/// A package that passed every check that needs no grants: load steps 1–6 of
/// `specs/002-plugin-permissions/data-model.md`.
pub struct CheckedPackage {
    component: Component,
    interfaces: Interfaces,
    manifest: Manifest,
    contract: ContractVersion,
    /// The package digest: SHA-256 of the whole file.
    digest: String,
    /// Each connector the manifest declares, loaded and checked (load step 6a).
    connectors: Vec<LoadedConnector>,
    /// The declared connectors step 6a refused, kept only by [`Runtime::inspect`].
    refused_connectors: Vec<(String, RuntimeError)>,
}

/// A connector the manifest declares, loaded and checked (load step 6a).
#[derive(Debug, Clone)]
pub struct LoadedConnector {
    pub(crate) id: String,
    /// `catalog`, or the canonical path of the local connector file.
    pub(crate) origin: String,
    pub(crate) connector: Arc<Connector>,
    pub(crate) required: bool,
    /// The operations the manifest declares for it, in its order.
    pub(crate) operations: Vec<String>,
    /// The manifest's `account`: the widget calls it as the connected account (feature 005).
    pub(crate) account: bool,
}

impl LoadedConnector {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// `catalog`, or the canonical path of the local connector file.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn connector(&self) -> &Connector {
        &self.connector
    }

    pub fn required(&self) -> bool {
        self.required
    }

    /// The operations the manifest declares for it, in its order.
    pub fn operations(&self) -> &[String] {
        &self.operations
    }

    /// What an approval records and compares of this connector.
    fn state(&self) -> ConnectorState {
        ConnectorState {
            id: self.id.clone(),
            origin: self.origin.clone(),
            version: self.connector.version().to_string(),
            required: self.required,
            destinations: self
                .connector
                .destinations()
                .iter()
                .map(|origin| origin.serialization())
                .collect(),
            operations: self
                .operations
                .iter()
                .map(|name| {
                    let digest = self
                        .connector
                        .operation_digest(name)
                        .expect("step 6a checked the operation exists");
                    (name.clone(), digest)
                })
                .collect(),
            account: self.connector.credential().map(|credential| AccountState {
                uses: self.account,
                required: credential.required(),
                credential: credential.digest(),
                access: credential.access().to_vec(),
            }),
        }
    }
}

impl CheckedPackage {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn contract(&self) -> Contract {
        self.contract.contract
    }

    /// The exact contract version the plugin is built for (`v1 (1.0.0)`).
    pub fn contract_version(&self) -> &ContractVersion {
        &self.contract
    }

    /// The package digest.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Each connector the manifest declares, as loaded.
    pub fn connectors(&self) -> &[LoadedConnector] {
        &self.connectors
    }

    /// The declared connectors that cannot be used, with why: only a package from
    /// [`Runtime::inspect`] has any, since [`Runtime::check`] refuses it.
    pub fn refused_connectors(&self) -> &[(String, RuntimeError)] {
        &self.refused_connectors
    }

    fn connector_states(&self) -> Vec<ConnectorState> {
        self.connectors.iter().map(LoadedConnector::state).collect()
    }

    /// The digest of everything an approval of this package covers (`data-model.md`, Approval
    /// digest), which `approve` needs for a widget that declares a connector or storage.
    pub fn approval_digest(&self) -> String {
        grants::approval_digest(&self.digest, &self.manifest, &self.connector_states())
    }

    /// Approves this package from `origin`: its required permissions and, for a widget that
    /// declares connectors or storage, every connector and the storage quota, only when
    /// `expected` is its approval digest (FR-014; feature 004, FR-008). Such a widget with no
    /// expected digest is refused as `approval-changed`, granting nothing; the CLI asks for
    /// `--expect` before.
    pub fn approve(
        &self,
        grants: &GrantStore,
        origin: &str,
        expected: Option<&str>,
    ) -> Result<Approval, RuntimeError> {
        if let Some((_, error)) = self.refused_connectors.first() {
            return Err(error.clone());
        }
        if self.connectors.is_empty() && self.manifest.storage().is_none() {
            return grants.approve(&self.manifest, origin, &self.digest);
        }
        let Some(expected) = expected else {
            return Err(RuntimeError::new(
                ErrorKind::ApprovalChanged,
                "a widget that declares connectors or storage is approved only with the approval digest of what was shown",
            ));
        };
        grants.approve_connected(
            &self.manifest,
            origin,
            &self.digest,
            &self.connector_states(),
            expected,
        )
    }

    /// Turns one optional connector of this package on or off, for packages from `origin`.
    pub fn set_connector(
        &self,
        grants: &GrantStore,
        origin: &str,
        id: &str,
        on: bool,
    ) -> Result<(), RuntimeError> {
        grants.set_connector(&self.manifest, origin, id, on)
    }

    /// Turns one optional permission of this package on or off, for packages from `origin`
    /// (FR-014). Refused as `optional-needs-v1` for a contract v0 plugin, which cannot answer
    /// "not granted", and otherwise as [`GrantStore::set_optional`] refuses.
    pub fn set_optional(
        &self,
        grants: &GrantStore,
        origin: &str,
        permission: &str,
        on: bool,
    ) -> Result<(), RuntimeError> {
        if self.contract.contract == Contract::V0 {
            return Err(RuntimeError::with_names(
                ErrorKind::OptionalNeedsV1,
                vec![permission.to_owned()],
                Some("contract v0 has no optional permissions; build against contract v1"),
            ));
        }
        grants.set_optional(&self.manifest, origin, permission, on)
    }
}

/// A loaded plugin, compiled once and instantiated afresh for every render call. Its linking
/// follows its grants: a change to them applies from the next call (FR-014, research R6).
pub struct Plugin {
    package: CheckedPackage,
    origin: String,
    grants: Arc<GrantStore>,
    linked: Mutex<Linked>,
    limits: Limits,
    clock: Arc<EpochClock>,
    network: network::Network,
    rates: Arc<Rates>,
    vault: Arc<vault::Vault>,
    kept: KeptStore,
    log: RequestLog,
    /// Where declared connectors are found, for [`Plugin::recheck`].
    catalog: catalog::Catalog,
    options: RuntimeOptions,
}

/// The plugin linked for one generation of its grant store, or the refusal that generation
/// produced.
struct Linked {
    generation: u64,
    prepared: Result<Arc<Prepared>, RuntimeError>,
}

/// A plugin linked for its contract version, ready to instantiate.
enum Prepared {
    V0(v0::WidgetPre<HostState>),
    V1(V1Pre),
}

/// A contract v1 plugin, linked, with its exports found by their exact names.
struct V1Pre {
    pre: InstancePre<HostState>,
    render: ComponentExportIndex,
    /// The `update` function, for a plugin that exports `updater`.
    update: Option<ComponentExportIndex>,
    /// The connectors granted when it was linked, for a plugin that imports `connectors`;
    /// `None` when it was linked to the stand-in, or imports nothing of them.
    table: Option<Arc<ConnectorTable>>,
    /// The connectors it uses with an account, each with the account connected when it was
    /// linked, for a plugin that imports `accounts`; `None` for the stand-in or no import.
    account_table: Option<Arc<ConnectorTable>>,
    /// The accounts record's vault id when it was linked.
    vault_id: Option<String>,
    /// The store bound when it was linked, for a plugin linked to the real `storage`, or why
    /// storage is unavailable; `None` when it was linked to the stand-in, or imports no storage.
    storage: Option<Result<Arc<storage::StoreBinding>, String>>,
}

/// The function `function` of the exported interface `interface`.
fn export_function(
    component: &Component,
    interface: &str,
    function: &str,
) -> Option<ComponentExportIndex> {
    let instance = component.get_export_index(None, interface)?;
    component.get_export_index(Some(&instance), function)
}

impl Runtime {
    /// Starts Wasmtime with every WebAssembly proposal the runtime does not need turned off.
    ///
    /// Fails only if Wasmtime cannot start on this machine.
    pub fn new(limits: Limits, options: RuntimeOptions) -> wasmtime::Result<Runtime> {
        Runtime::start(
            limits,
            options,
            catalog::Catalog::built_in(),
            network::Network::production(),
            vault::Backend::System,
        )
    }

    /// A runtime whose connector requests go through the test seam instead of DNS and
    /// Mozilla's roots, and whose catalog has `test.catalog` added (research R15). Only the
    /// runtime's own tests can build one.
    #[cfg(feature = "test-network")]
    pub fn with_test_network(
        limits: Limits,
        options: RuntimeOptions,
        test: TestNetwork,
    ) -> wasmtime::Result<Runtime> {
        let catalog = catalog::Catalog::with_extra(&test.catalog)?;
        Runtime::start(
            limits,
            options,
            catalog,
            network::Network::with_seam(test),
            vault::Backend::mock(),
        )
    }

    fn start(
        limits: Limits,
        mut options: RuntimeOptions,
        catalog: catalog::Catalog,
        network: network::Network,
        backend: vault::Backend,
    ) -> wasmtime::Result<Runtime> {
        // A store's path is always absolute: a relative one could be read as a URI, or move
        // with the working directory (specs/004-plugin-storage/research.md, R4)
        if let Ok(absolute) = std::path::absolute(&options.data_dir) {
            options.data_dir = absolute;
        }
        let mut config = Config::new();
        // Turned off explicitly, so a Wasmtime upgrade that changes a default does not widen
        // what plugins can use. Function references, exceptions, threads and component-model
        // async cannot be turned on at all: their Wasmtime features are not compiled in
        // (runtime/Cargo.toml).
        config
            .wasm_gc(false)
            .wasm_stack_switching(false)
            .wasm_shared_everything_threads(false)
            .wasm_tail_call(false)
            .wasm_memory64(false)
            .wasm_multi_memory(false)
            .wasm_relaxed_simd(false)
            .wasm_wide_arithmetic(false)
            .wasm_extended_const(false)
            .wasm_custom_page_sizes(false)
            .wasm_compact_imports(false)
            .wasm_branch_hinting(false)
            .wasm_component_model_gc(false)
            .wasm_component_model_memory64(false)
            .wasm_component_model_error_context(false)
            .wasm_component_model_map(false)
            .wasm_component_model_fixed_length_lists(false)
            .wasm_component_model_implements(false)
            .wasm_component_model_canonical_names(false)
            .wasm_component_model_accessors(false);
        // The CPU budget, deterministic on every system, and the wall-clock time limit (R2)
        config.consume_fuel(true).epoch_interruption(true);
        let engine = Engine::new(&config)?;
        let clock = Arc::new(EpochClock::new());
        start_epoch_ticker(&engine, Arc::clone(&clock))?;

        let vault = Arc::new(vault::Vault::new(backend, limits.max_secret_bytes));
        Ok(Runtime {
            engine,
            limits,
            clock,
            options,
            catalog,
            network,
            rates: Arc::default(),
            vault,
        })
    }

    fn connectors(&self) -> Connectors<'_> {
        Connectors {
            catalog: &self.catalog,
            options: &self.options,
        }
    }

    /// What connecting and disconnecting accounts need (`specs/005-plugin-credentials/
    /// data-model.md`, Sign-in).
    fn signin_context(&self) -> signin::Context<'_> {
        signin::Context {
            accounts: accounts::AccountStore::open(self.options.data_dir()),
            vault: &self.vault,
            session: self.network.session(&self.limits),
            log: RequestLog::new(self.options.data_dir(), &self.limits),
            limits: &self.limits,
        }
    }

    /// Connects an account to the connector `connector_id` by signing in with the authorization
    /// code and PKCE (FR-009): `open` shows the person the sign-in address in the system
    /// browser. `client` is the client registration's id, remembered for the connector; without
    /// it, the one remembered from before. Only the host's own controls call this (Principle VII,
    /// constitution v2.4.0); no plugin can.
    pub fn connect_account(
        &self,
        connector_id: &str,
        client: Option<&str>,
        open: &(dyn Fn(&url::Url) + Sync),
    ) -> Result<accounts::Connected, RuntimeError> {
        let (connector, origin) = self.connectors().find(connector_id)?;
        signin::connect(&self.signin_context(), &connector, &origin, client, open)
    }

    /// Connects a key the maintainer typed to the connector `connector_id` (FR-010).
    pub fn connect_key(
        &self,
        connector_id: &str,
        key: &str,
    ) -> Result<accounts::Connected, RuntimeError> {
        let (connector, origin) = self.connectors().find(connector_id)?;
        signin::connect_key(&self.signin_context(), &connector, &origin, key)
    }

    /// Every connected account, with whether its connector's file still exists (FR-014).
    pub fn list_accounts(&self) -> Result<Vec<accounts::Account>, RuntimeError> {
        accounts::AccountStore::open(self.options.data_dir()).list()
    }

    /// The connector `connector_id` as found now, for the tool's output: its declaration and
    /// origin.
    pub fn find_connector(&self, connector_id: &str) -> Result<(Arc<Connector>, String), RuntimeError> {
        self.connectors().find(connector_id)
    }

    /// Disconnects the account of `connector_id` from `origin` (FR-013, FR-014): deletes its
    /// credential from the keychain and the account from the record under the record's lock,
    /// deletes at once the kept data of every widget that used it, then asks the service to
    /// revoke the token when the connector says where. `origin` defaults to the connector's
    /// file as found now, and must be given when that file is gone. `None` when nothing was
    /// connected.
    pub fn disconnect_account(
        &self,
        connector_id: &str,
        origin: Option<&str>,
    ) -> Result<Option<Disconnected>, RuntimeError> {
        let found = self.connectors().find(connector_id).ok();
        let origin = match (origin, &found) {
            (Some(origin), _) => origin.to_owned(),
            (None, Some((_, origin))) => origin.clone(),
            (None, None) => {
                return Err(RuntimeError::new(
                    ErrorKind::AccountNotConnected,
                    format!(
                        "{connector_id} cannot be found now; give the origin of its account with --origin"
                    ),
                ));
            }
        };
        let context = self.signin_context();
        let mut token = None;
        let removed = context.accounts.disconnect(connector_id, &origin, |account, vault_id| {
            let user = vault::entry_user(vault_id, connector_id, &origin);
            let wait = self.limits.vault_wait_tool;
            if let Ok(vault::Stored::Account { access, .. }) = self.vault.read(&user).within(wait) {
                token = Some(access);
            }
            let _ = account;
            self.vault
                .delete(&user)
                .within(wait)
                .map(drop)
                .map_err(|error| RuntimeError::new(ErrorKind::KeychainUnavailable, error.to_string()))
        })?;
        let Some(account) = removed else {
            return Ok(None);
        };
        let kept_discarded = KeptStore::new(self.options.data_dir(), self.limits.max_update_output_bytes)
            .discard_account(connector_id, &account.account_id)
            .unwrap_or_default();
        let revoked = match (&found, &token, &account.client) {
            (Some((connector, found_origin)), Some(token), Some(client)) if *found_origin == origin => {
                signin::revoke(&context, connector, &client.client_id, token)
            }
            _ => None,
        };
        let revoke_page = found
            .as_ref()
            .and_then(|(connector, _)| connector.credential())
            .and_then(|credential| credential.account())
            .and_then(|account| account.revoke_page())
            .map(|page| page.to_string());
        Ok(Some(Disconnected {
            account,
            revoked,
            revoke_page,
            kept_discarded,
        }))
    }

    /// Discards every kept data file of the plugin `id`, as removing its approval or turning
    /// off one of its connectors does (FR-010). Returns how many there were.
    pub fn discard_kept_data(&self, id: &manifest::PluginId) -> std::io::Result<usize> {
        KeptStore::new(self.options.data_dir(), self.limits.max_update_output_bytes)
            .discard(id.as_str())
    }

    /// The last `n` entries of the request log, oldest first, of `plugin` only when given
    /// (FR-009). Only the host's own controls read it; every text in it is a plugin's or a
    /// connector's, to escape before it is shown.
    pub fn read_requests(
        &self,
        n: usize,
        plugin: Option<&str>,
    ) -> std::io::Result<Vec<requests::Entry>> {
        RequestLog::new(self.options.data_dir(), &self.limits).read_last(n, plugin)
    }

    /// The usage of the store of `id` from `origin`, in bytes: 0 when it has none yet. Reads
    /// the store without changing or creating anything; refused as `storage-failed` when the
    /// store cannot be read, naming why.
    pub fn storage_used_bytes(
        &self,
        id: &manifest::PluginId,
        origin: &str,
    ) -> Result<u64, RuntimeError> {
        storage::used_bytes(self.options.data_dir(), id.as_str(), origin, &self.limits)
            .map_err(|reason| RuntimeError::new(ErrorKind::StorageFailed, reason))
    }

    /// Every store in the data directory, sorted by id then origin, for `runtime storage list`.
    /// Changes nothing.
    pub fn list_stores(&self) -> Vec<storage::StoreInfo> {
        storage::list_stores(self.options.data_dir(), &self.limits)
    }

    /// What the store of `id` from `origin` holds, read-only; `None` when it has none yet.
    pub fn show_store(
        &self,
        id: &manifest::PluginId,
        origin: &str,
    ) -> Result<Option<storage::StoreView>, RuntimeError> {
        storage::show_store(self.options.data_dir(), id.as_str(), origin, &self.limits)
    }

    /// Empties the store of `id` from `origin`, or deletes its file when it is damaged or the
    /// origin's file is gone. Only the host's own controls reach a store this way (spec FR-014).
    pub fn clear_store(&self, id: &str, origin: &str) -> Result<storage::Cleared, RuntimeError> {
        storage::clear_store(self.options.data_dir(), id, origin, &self.limits)
    }

    /// Checks a plugin package without its grants: size, manifest, compilation, contract and
    /// imports (load steps 1–6). A failure stops at the first step that refuses the package,
    /// with that step's error kind.
    pub fn check(&self, bytes: &[u8]) -> Result<CheckedPackage, RuntimeError> {
        let package = self.inspect(bytes)?;
        match package.refused_connectors.first() {
            Some((_, error)) => Err(error.clone()),
            None => Ok(package),
        }
    }

    /// As [`check`](Self::check), keeping the declared connectors that step 6a refuses beside
    /// the package instead of refusing it, so `permissions` can show each refusal in its place.
    /// A package with such a connector can neither be approved nor loaded.
    pub fn inspect(&self, bytes: &[u8]) -> Result<CheckedPackage, RuntimeError> {
        // Step 1: size and preamble
        if bytes.len() > self.limits.max_package_bytes {
            return Err(RuntimeError::new(
                ErrorKind::InvalidPackage,
                // Not the exact size: callers may stop reading right after the limit.
                format!(
                    "the package is larger than the limit of {} bytes",
                    self.limits.max_package_bytes
                ),
            ));
        }
        if !bytes.starts_with(&COMPONENT_PREAMBLE) {
            return Err(RuntimeError::new(
                ErrorKind::InvalidPackage,
                "not a WebAssembly component",
            ));
        }
        // Steps 2 and 3: the one manifest, before the compile, which costs far more
        let manifest = Manifest::parse(package::manifest_section(bytes)?)?;
        // Step 3a: a storage quota within this host's cap, which may be lower than the schema's
        self.check_quota(&manifest)?;
        // Step 4: from_binary never accepts the text format, whatever Wasmtime features are
        // enabled.
        let component = Component::from_binary(&self.engine, bytes)
            .map_err(|error| RuntimeError::new(ErrorKind::InvalidPackage, format!("{error:#}")))?;
        // Steps 5 and 6: on names, before linking, so every refused import is named
        let interfaces = Interfaces::of(&self.engine, &component);
        let contract = permissions::check_component(&interfaces, &manifest)?;
        // Step 6a: each declared connector, from the catalog or, in development, a local file
        let mut connectors = Vec::new();
        let mut refused_connectors = Vec::new();
        for request in manifest.connectors() {
            match self.connectors().resolve(request) {
                Ok(loaded) => connectors.push(loaded),
                Err(error) => refused_connectors.push((request.id.clone(), error)),
            }
        }
        Ok(CheckedPackage {
            component,
            interfaces,
            manifest,
            contract,
            digest: package::digest(bytes),
            connectors,
            refused_connectors,
        })
    }

    /// Load step 3a (`specs/004-plugin-storage/data-model.md`, Load decision).
    fn check_quota(&self, manifest: &Manifest) -> Result<(), RuntimeError> {
        let Some(storage) = manifest.storage() else {
            return Ok(());
        };
        let cap = self.limits.max_storage_quota_bytes;
        if storage.quota_bytes <= cap {
            return Ok(());
        }
        let index = manifest
            .permissions()
            .iter()
            .position(|permission| permission.name == "storage")
            .unwrap_or_default();
        Err(RuntimeError::new(
            ErrorKind::InvalidManifest,
            format!(
                "permissions[{index}].quota_bytes: {} is over this host's cap; a quota is from {} to {cap} usage bytes",
                storage.quota_bytes, self.limits.storage_entry_cost_bytes
            ),
        ))
    }

    /// Checks a connector file as this runtime would load it as `local/<name>` from
    /// `<name>.json`: the format, the rules of a local connector, its id against its file name,
    /// and the hosts the catalog reserves. For `runtime check-connector`, for authors.
    pub fn check_local_connector(
        &self,
        bytes: &[u8],
        name: &str,
    ) -> Result<Connector, RuntimeError> {
        let connector = Connector::parse(bytes, Source::Local)?;
        let id = format!("local/{name}");
        if connector.id() != id {
            return Err(RuntimeError::with_names(
                ErrorKind::InvalidConnector,
                vec![connector.id().to_owned()],
                Some(&format!("the file {name}.json must carry the id {id}")),
            ));
        }
        self.connectors().check_not_reserved(&connector)?;
        Ok(connector)
    }

    /// Checks a plugin package and its grants, and links it with exactly the capabilities its
    /// manifest declares and its approval grants (load steps 1–9).
    ///
    /// `origin` is where the package came from: in the CLI, its file's canonical path. An
    /// approval applies only to packages from the origin it was given for, so another package
    /// that claims an approved id gets none of its grants (FR-020).
    pub fn load(
        &self,
        bytes: &[u8],
        origin: &str,
        grants: Arc<GrantStore>,
    ) -> Result<Plugin, RuntimeError> {
        let package = self.check(bytes)?;
        // Read before the grants, so a change made while linking is seen on the next call
        let generation = grants.generation();
        let prepared = prepare(&package, origin, &grants, self.options.data_dir())?;
        let plugin = Plugin {
            package,
            origin: origin.to_owned(),
            grants,
            linked: Mutex::new(Linked {
                generation,
                prepared: Ok(Arc::new(prepared)),
            }),
            limits: self.limits.clone(),
            clock: Arc::clone(&self.clock),
            network: self.network.clone(),
            rates: Arc::clone(&self.rates),
            vault: Arc::clone(&self.vault),
            kept: KeptStore::new(self.options.data_dir(), self.limits.max_update_output_bytes),
            log: RequestLog::new(self.options.data_dir(), &self.limits),
            catalog: self.catalog.clone(),
            options: self.options.clone(),
        };
        // Step 7a: kept data that no longer matches the package or its approval goes now
        plugin.kept_data();
        Ok(plugin)
    }
}

/// Load step 6a, for the runtime and for a loaded plugin's [`Plugin::recheck`]: where declared
/// connectors are found and the rules they follow.
pub(crate) struct Connectors<'a> {
    catalog: &'a catalog::Catalog,
    options: &'a RuntimeOptions,
}

impl Connectors<'_> {
    /// The connector `id`, from the catalog or, in development, its local file, with its origin:
    /// `catalog`, or the file's canonical path. Repository ids are not verified yet (FR-004).
    pub(crate) fn find(&self, id: &str) -> Result<(Arc<Connector>, String), RuntimeError> {
        let names = || vec![id.to_owned()];
        let (connector, origin) = if id.starts_with("catalog/") {
            let connector = self.catalog.get(id).ok_or_else(|| {
                RuntimeError::with_names(
                    ErrorKind::ConnectorUnavailable,
                    names(),
                    Some("it is not in the catalog this host was built with"),
                )
            })?;
            (Arc::clone(connector), "catalog".to_owned())
        } else if let Some(name) = id.strip_prefix("local/") {
            if !self.options.is_development() {
                return Err(RuntimeError::with_names(
                    ErrorKind::DestinationNotVerified,
                    names(),
                    Some(
                        "local connectors load only in development mode, from the development command-line tool",
                    ),
                ));
            }
            self.local_connector(id, name)?
        } else {
            return Err(RuntimeError::with_names(
                ErrorKind::DestinationNotVerified,
                names(),
                Some(
                    "connectors from repositories, whose destinations their owners verify, arrive with installation (Phase 3)",
                ),
            ));
        };
        Ok((connector, origin))
    }

    /// Load step 6a for one declared connector: found, parsed, following the destination
    /// rules, of the declared major, with every declared operation
    /// (`specs/003-plugin-connectors/data-model.md`, Load decision).
    pub(crate) fn resolve(
        &self,
        request: &manifest::ConnectorRequest,
    ) -> Result<LoadedConnector, RuntimeError> {
        let id = request.id.as_str();
        let names = || vec![id.to_owned()];
        let (connector, origin) = self.find(id)?;
        if connector.version().major != u64::from(request.major) {
            return Err(RuntimeError::with_names(
                ErrorKind::ConnectorUnavailable,
                names(),
                Some(&format!(
                    "the widget needs major version {}, and the connector is version {}",
                    request.major,
                    connector.version()
                )),
            ));
        }
        if let Some(missing) = request
            .operations
            .iter()
            .find(|name| connector.operation(name).is_none())
        {
            return Err(RuntimeError::with_names(
                ErrorKind::ConnectorUnavailable,
                names(),
                Some(&format!(
                    "the widget declares the operation `{missing}`, which this version of the connector does not have"
                )),
            ));
        }
        // Account use (`specs/005-plugin-credentials/data-model.md`, Load decision, step 6a)
        match connector.credential() {
            None if request.account => {
                return Err(RuntimeError::with_names(
                    ErrorKind::InvalidManifest,
                    names(),
                    Some("the manifest says the widget uses this connector's account, and the connector declares no credential"),
                ));
            }
            Some(credential) if credential.required() && !request.account => {
                return Err(RuntimeError::with_names(
                    ErrorKind::AccountUseNotDeclared,
                    names(),
                    Some("this connector's credential is required, so the widget must declare \"account\": true and call it through accounts"),
                ));
            }
            _ => {}
        }
        Ok(LoadedConnector {
            id: id.to_owned(),
            origin,
            connector,
            required: request.required,
            operations: request.operations.clone(),
            account: request.account,
        })
    }

    /// `local/<name>` from `<connectors dir>/<name>.json`: parsed as a local connector, carrying
    /// that id, and claiming no host the catalog reserves. Its origin is the file's canonical
    /// path, which an approval belongs to (FR-018).
    fn local_connector(
        &self,
        id: &str,
        name: &str,
    ) -> Result<(Arc<Connector>, String), RuntimeError> {
        let names = || vec![id.to_owned()];
        let path = self
            .options
            .local_connectors_dir()
            .join(format!("{name}.json"));
        let bytes = read_capped(&path, connector::MAX_CONNECTOR_BYTES).map_err(|why| {
            RuntimeError::with_names(
                ErrorKind::ConnectorUnavailable,
                names(),
                Some(&format!("{}: {why}", path.display())),
            )
        })?;
        let connector = Connector::parse(&bytes, Source::Local)?;
        if connector.id() != id {
            return Err(RuntimeError::with_names(
                ErrorKind::InvalidConnector,
                names(),
                Some(&format!(
                    "{} says its id is `{}`; the file of {id} must carry that id",
                    path.display(),
                    connector.id()
                )),
            ));
        }
        self.check_not_reserved(&connector)?;
        let origin = std::fs::canonicalize(&path)
            .ok()
            .and_then(|path| path.to_str().map(str::to_owned))
            .ok_or_else(|| {
                RuntimeError::with_names(
                    ErrorKind::ConnectorUnavailable,
                    names(),
                    Some(&format!("{} has no canonical UTF-8 path", path.display())),
                )
            })?;
        Ok((Arc::new(connector), origin))
    }

    /// A connector outside the catalog may not declare a host the catalog reserves (FR-005).
    pub(crate) fn check_not_reserved(&self, connector: &Connector) -> Result<(), RuntimeError> {
        for origin in connector.destinations() {
            if let Some((owner, domain)) = self.catalog.reserving(origin.host()) {
                return Err(RuntimeError::with_names(
                    ErrorKind::DestinationReserved,
                    vec![origin.host().to_owned()],
                    Some(&format!(
                        "{} declares it, and {domain} with every host under it is reserved to the catalog's {owner}",
                        connector.id()
                    )),
                ));
            }
        }
        Ok(())
    }
}

/// Load steps 7–9: the plugin's approval from `origin`, pruning, then a linker built for this
/// plugin alone, holding the real interface of each granted import and the stand-in of each
/// optional one that is off. Nothing else is ever linked (Principle II).
fn prepare(
    package: &CheckedPackage,
    origin: &str,
    grants: &GrantStore,
    data_dir: &Path,
) -> Result<Prepared, RuntimeError> {
    let manifest = &package.manifest;
    let entry = grants.entry(manifest.id())?;
    let decision = permissions::decide(
        package.contract.contract,
        &package.interfaces,
        manifest,
        origin,
        entry.as_ref(),
    )?;
    // Step 7 for connectors: each required one, and each optional one that is on, as approved
    if !package.connectors.is_empty() {
        let entry = entry
            .as_ref()
            .expect("step 7 refused a plugin without an approval");
        let states = package.connector_states();
        let changes = entry.connector_changes(manifest, &states);
        if !changes.is_empty() {
            return Err(RuntimeError::with_names(
                ErrorKind::NeedsNewApproval,
                changes,
                Some(&format!(
                    "{} was approved, and its connectors changed since; approve it again",
                    manifest.id()
                )),
            ));
        }
        grants.refresh_connector_versions(manifest, origin, &states)?;
    }
    // Step 7 for storage: required, or optional and on, with a quota the user approved
    // (specs/004-plugin-storage/data-model.md, Load decision)
    if let Some(change) = entry
        .as_ref()
        .and_then(|entry| entry.storage_change(manifest))
    {
        return Err(RuntimeError::with_names(
            ErrorKind::NeedsNewApproval,
            vec!["storage".to_owned()],
            Some(&format!("{change}; approve {} again", manifest.id())),
        ));
    }
    // Step 8: permissions and connectors this manifest no longer declares leave the approval
    let undeclared_connectors = entry.as_ref().is_some_and(|entry| {
        entry
            .connectors
            .keys()
            .any(|id| manifest.connector(id).is_none())
    });
    if !decision.prune.is_empty() || undeclared_connectors {
        grants.prune(manifest, origin)?;
    }

    let mismatch = |error: wasmtime::Error| {
        RuntimeError::new(ErrorKind::ContractMismatch, format!("{error:#}"))
    };
    let mut linker = Linker::new(package.component.engine());
    let plan = &decision.plan;
    for permission in &plan.real {
        host::link(&mut linker, plan.contract, permission, host::Link::Real).map_err(mismatch)?;
    }
    for permission in &plan.stand_in {
        host::link(&mut linker, plan.contract, permission, host::Link::StandIn)
            .map_err(mismatch)?;
    }
    // Step 9 for connectors: the real interface over a table of the granted ones, built now,
    // or the stand-in when none is granted (Principle II, research R12)
    let table = if plan.connectors {
        let loaded: Vec<(String, Arc<Connector>)> = package
            .connectors
            .iter()
            .map(|loaded| (loaded.id.clone(), Arc::clone(&loaded.connector)))
            .collect();
        let entry = entry
            .as_ref()
            .expect("step 7 refused a plugin without an approval");
        let table = ConnectorTable::build(manifest, &loaded, entry).map(Arc::new);
        let link = if table.is_some() {
            host::Link::Real
        } else {
            host::Link::StandIn
        };
        host::link_connectors(&mut linker, link).map_err(mismatch)?;
        table
    } else {
        None
    };
    // Step 9 for accounts: the real interface over a table of the connectors used with an
    // account, each with the account connected now, or the stand-in (research R9, R11 step 0)
    let accounts = accounts::AccountStore::open(data_dir);
    let (account_table, vault_id) = if plan.accounts {
        let loaded: Vec<(String, String, Arc<Connector>)> = package
            .connectors
            .iter()
            .map(|loaded| {
                (
                    loaded.id.clone(),
                    loaded.origin.clone(),
                    Arc::clone(&loaded.connector),
                )
            })
            .collect();
        let entry = entry
            .as_ref()
            .expect("step 7 refused a plugin without an approval");
        let table = ConnectorTable::build_accounts(manifest, &loaded, entry, |id, origin| {
            accounts.get(id, origin).ok().flatten()
        })
        .map(Arc::new);
        let link = if table.is_some() {
            host::Link::Real
        } else {
            host::Link::StandIn
        };
        host::link_accounts(&mut linker, link).map_err(mismatch)?;
        (table, accounts.existing_vault_id().ok().flatten())
    } else {
        (None, None)
    };
    // Steps 8a and 9 for storage: the store binding, from the plugin's id and origin, for a
    // plugin linked to the real interface; never from anything a call says (research R3)
    let storage = if plan.real.contains(&"storage") {
        let declared = manifest
            .storage()
            .expect("storage is linked only when the manifest declares it");
        let approved = entry
            .as_ref()
            .and_then(|entry| entry.storage_quota_bytes)
            .unwrap_or(declared.quota_bytes);
        match storage::storage_dir(data_dir) {
            Ok(dir) => Some(Ok(Arc::new(storage::StoreBinding::new(
                &dir,
                manifest.id().as_str(),
                origin,
                declared.quota_bytes,
                approved,
            )))),
            Err(reason) if declared.required => {
                return Err(RuntimeError::with_names(
                    ErrorKind::StorageFailed,
                    vec!["storage".to_owned()],
                    Some(&format!("its required storage is unavailable: {reason}")),
                ));
            }
            // Optional storage that is on: linked, and its calls fail as unavailable
            Err(reason) => Some(Err(reason)),
        }
    } else {
        None
    };
    let pre = linker
        .instantiate_pre(&package.component)
        .map_err(mismatch)?;
    Ok(match plan.contract {
        Contract::V0 => Prepared::V0(v0::WidgetPre::new(pre).map_err(mismatch)?),
        Contract::V1 => {
            let interfaces = &package.interfaces;
            let render = interfaces
                .renderer()
                .and_then(|renderer| export_function(&package.component, renderer, "render"))
                .ok_or_else(|| {
                    RuntimeError::new(ErrorKind::ContractMismatch, "exports no render function")
                })?;
            let update = interfaces
                .updater()
                .and_then(|updater| export_function(&package.component, updater, "update"));
            Prepared::V1(V1Pre {
                pre,
                render,
                update,
                table,
                account_table,
                vault_id,
                storage,
            })
        }
    })
}

impl Plugin {
    /// The manifest the plugin was loaded with.
    pub fn manifest(&self) -> &Manifest {
        &self.package.manifest
    }

    /// The contract version the plugin implements.
    pub fn contract(&self) -> Contract {
        self.package.contract.contract
    }

    /// The exact contract version the plugin is built for.
    pub fn contract_version(&self) -> &ContractVersion {
        &self.package.contract
    }

    /// Calls the plugin's `render` with a JSON configuration, in a fresh instance that is
    /// dropped when the call ends, so nothing survives from one call to the next (FR-008).
    ///
    /// A widget that exports `updater` gets the document of `data-model.md` (What render
    /// receives): its configuration, its kept data, when it was fetched and how the last update
    /// went. Render never causes a network request: a connector call from it is refused.
    ///
    /// If the grants changed since the plugin was last linked, it is linked again first, with
    /// the compiled component kept; if they now refuse it, the call fails with that refusal and
    /// nothing is instantiated. A call already running keeps the instance it started with.
    pub fn render(&self, config: &str) -> RenderReport {
        let prepared = match self.current() {
            Ok(prepared) => prepared,
            Err(error) => {
                return RenderReport {
                    result: Err(error),
                    logs: Vec::new(),
                    dropped_logs: 0,
                    storage: None,
                };
            }
        };
        if self.package.interfaces.updater().is_some() {
            let config: serde_json::Value = serde_json::from_str(config)
                .unwrap_or_else(|_| serde_json::Value::String(config.to_owned()));
            let document = kept::render_document(config, self.kept_data().as_ref()).to_string();
            return self.render_prepared(&prepared, &document);
        }
        self.render_prepared(&prepared, config)
    }

    /// Renders `document` as it is, for [`render_with_kept`](Self::render_with_kept).
    fn render_document(&self, document: &str) -> RenderReport {
        match self.current() {
            Ok(prepared) => self.render_prepared(&prepared, document),
            Err(error) => RenderReport {
                result: Err(error),
                logs: Vec::new(),
                dropped_logs: 0,
                storage: None,
            },
        }
    }

    fn render_prepared(&self, prepared: &Prepared, config: &str) -> RenderReport {
        let mut store = self.store(prepared, Phase::Render);
        let result = self
            .start_limits(&mut store, self.limits.time_limit)
            .and_then(|()| self.call(prepared, &mut store, config));
        let mut state = store.into_data();
        state.run.finish();
        // A render keeps nothing: its read transaction just ends
        let storage = state.storage.take().and_then(|run| run.finish(true).ok());
        RenderReport {
            result,
            logs: state.logs,
            dropped_logs: state.dropped_logs,
            storage,
        }
    }

    /// Calls the plugin's `update` with its JSON configuration, in a fresh instance, with the
    /// update's own time limit and render's CPU and memory caps (FR-010). Its connector calls go
    /// through the host's network module. On success the data it returns is kept, with the
    /// time; on failure the previous data stays and the failure is recorded.
    pub fn update(&self, config: &str) -> UpdateReport {
        let failed = |error: RuntimeError| UpdateReport {
            result: Err(error),
            logs: Vec::new(),
            dropped_logs: 0,
            calls: CallCounts::default(),
            storage: None,
        };
        let prepared = match self.current() {
            Ok(prepared) => prepared,
            Err(error) => return failed(error),
        };
        let update = match &*prepared {
            Prepared::V1(V1Pre {
                update: Some(update),
                ..
            }) => *update,
            _ => {
                return failed(RuntimeError::new(
                    ErrorKind::MissingUpdater,
                    format!(
                        "{} exports no updater, so it has nothing to update",
                        self.manifest().id()
                    ),
                ));
            }
        };
        let mut store = self.store(&prepared, Phase::Update);
        let result = self
            .start_limits(&mut store, self.limits.update_time_limit)
            .and_then(|()| self.call_update(&prepared, &mut store, update, config));
        let mut state = store.into_data();
        state.run.finish();
        // The store first: its writes are committed only when the update succeeded, and a
        // commit that fails fails the update, before the kept data is recorded
        // (specs/004-plugin-storage/research.md, R7)
        let (result, storage) = match state.storage.take() {
            None => (result, None),
            Some(run) => match run.finish(result.is_ok()) {
                Ok(counts) => (result, Some(counts)),
                Err(error) => (Err(error), None),
            },
        };
        let now = calls::now_ms();
        if let Some(key) = self.kept_key() {
            // The kept data is the host's record of the update: a write that fails leaves the
            // previous data, as a failed update does
            let _ = match &result {
                Ok(data) => self.kept.record_success(&key, data.clone(), now),
                Err(error) => self.kept.record_failure(&key, &error.to_string(), now),
            };
        }
        UpdateReport {
            result,
            logs: state.logs,
            dropped_logs: state.dropped_logs,
            calls: state.run.counts,
            storage,
        }
    }

    /// The connectors the plugin can call, as linked now: the table's keys, or `None` when the
    /// stand-in is linked or the plugin imports no connector. For the white-box hostile case
    /// `connector-table`, which no plugin can observe (Principle VIII).
    #[cfg(feature = "test-network")]
    pub fn connector_table(&self) -> Option<Vec<String>> {
        match &*self.current().ok()? {
            Prepared::V1(V1Pre {
                table: Some(table), ..
            }) => Some(table.keys()),
            _ => None,
        }
    }

    /// The file of the store the plugin is linked with, or `None` when it is linked to the
    /// stand-in or imports no storage. For the white-box hostile case `store-binding`, which no
    /// plugin can observe (Principle VIII).
    #[cfg(feature = "test-network")]
    pub fn store_binding(&self) -> Option<PathBuf> {
        match &*self.current().ok()? {
            Prepared::V1(V1Pre {
                storage: Some(Ok(binding)),
                ..
            }) => Some(binding.file.clone()),
            _ => None,
        }
    }

    /// A store for one call, with the state its connector calls need.
    fn store(&self, prepared: &Prepared, phase: Phase) -> Store<HostState> {
        let table = match prepared {
            Prepared::V1(V1Pre {
                table: Some(table), ..
            }) => Some(Arc::clone(table)),
            _ => None,
        };
        let (account_table, vault_id) = match prepared {
            Prepared::V1(V1Pre {
                account_table: Some(table),
                vault_id,
                ..
            }) => (Some(Arc::clone(table)), vault_id.clone()),
            _ => (None, None),
        };
        let log = (!self.package.connectors.is_empty()).then(|| self.log.clone());
        let run = RunLog::new(self.manifest().id(), log, &self.limits);
        let calls = table.map(|table| Calls {
            plugin: self.manifest().id().clone(),
            origin: self.origin.clone(),
            grants: Arc::clone(&self.grants),
            table,
            phase,
            session: (phase == Phase::Update).then(|| self.network.session(&self.limits)),
            deadline: Instant::now() + self.limits.update_time_limit,
            update_bytes_left: self.limits.max_update_response_bytes,
            limits: self.limits.clone(),
            rates: Arc::clone(&self.rates),
            account: None,
        });
        let account_calls = account_table.map(|table| Calls {
            plugin: self.manifest().id().clone(),
            origin: self.origin.clone(),
            grants: Arc::clone(&self.grants),
            table,
            phase,
            session: (phase == Phase::Update).then(|| self.network.session(&self.limits)),
            deadline: Instant::now() + self.limits.update_time_limit,
            update_bytes_left: self.limits.max_update_response_bytes,
            limits: self.limits.clone(),
            rates: Arc::clone(&self.rates),
            account: Some(calls::AccountAccess {
                store: accounts::AccountStore::open(self.options.data_dir()),
                vault: Arc::clone(&self.vault),
                vault_id,
                secrets: std::collections::HashMap::new(),
            }),
        });
        let mut state = HostState::new(&self.limits, run, calls);
        state.account_calls = account_calls;
        if let Prepared::V1(V1Pre {
            storage: Some(binding),
            ..
        }) = prepared
        {
            let time_limit = match phase {
                Phase::Render => self.limits.time_limit,
                Phase::Update => self.limits.update_time_limit,
            };
            state.storage = Some(StorageRun::new(
                binding.clone(),
                phase,
                Arc::clone(&self.grants),
                &self.limits,
                Instant::now() + time_limit,
            ));
        }
        let mut store = Store::new(self.package.component.engine(), state);
        store.limiter(|state| &mut state.limiter);
        store
    }

    /// What the kept data of this package belongs to now, for a widget that exports `updater`:
    /// its package, its approval digest and the connectors that are on.
    fn kept_key(&self) -> Option<kept::Key> {
        self.package.interfaces.updater()?;
        let entry = self.grants.entry(self.manifest().id()).ok().flatten()?;
        let mut connectors_on: Vec<String> = entry
            .connectors
            .iter()
            .filter(|(_, grant)| grant.granted)
            .map(|(id, _)| id.clone())
            .collect();
        connectors_on.sort();
        // The accounts its account calls act as, from the table it was linked with
        let accounts = match self.linked.lock().unwrap_or_else(PoisonError::into_inner).prepared.as_deref() {
            Ok(Prepared::V1(V1Pre {
                account_table: Some(table),
                ..
            })) => table.account_ids(),
            _ => std::collections::BTreeMap::new(),
        };
        Some(kept::Key {
            id: self.manifest().id().to_string(),
            origin: self.origin.clone(),
            digest: self.package.digest.clone(),
            approval: self.package.approval_digest(),
            connectors_on,
            accounts,
        })
    }

    /// The kept data, discarding it first when it no longer matches (load step 7a). A damaged
    /// file counts as no data.
    fn kept_data(&self) -> Option<kept::Kept> {
        self.package.interfaces.updater()?;
        let Some(key) = self.kept_key() else {
            // No approval any more, removed by any process: its data goes with it (FR-010)
            let _ = self.kept.discard(self.manifest().id().as_str());
            return None;
        };
        match self.kept.read(&key) {
            Found::Current(kept) => kept,
            Found::Discarded(_) | Found::Damaged(_) => None,
        }
    }

    /// Runs again every check of a load but the compile and the linking: the manifest section
    /// and the manifest, the contract and the imports, and the grants (load steps 2, 3 and
    /// 5–8), on the package bytes it was loaded from. For `runtime bench` (SC-007).
    pub fn recheck(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        let manifest = Manifest::parse(package::manifest_section(bytes)?)?;
        let contract = permissions::check_component(&self.package.interfaces, &manifest)?;
        // Step 6a: each declared connector found and checked again (SC-007)
        let connectors = Connectors {
            catalog: &self.catalog,
            options: &self.options,
        };
        let loaded = manifest
            .connectors()
            .iter()
            .map(|request| connectors.resolve(request))
            .collect::<Result<Vec<_>, _>>()?;
        let entry = self.grants.entry(manifest.id())?;
        permissions::decide(
            contract.contract,
            &self.package.interfaces,
            &manifest,
            &self.origin,
            entry.as_ref(),
        )?;
        // Step 7 for connectors
        if let Some(entry) = &entry {
            let states: Vec<ConnectorState> = loaded.iter().map(LoadedConnector::state).collect();
            let changes = entry.connector_changes(&manifest, &states);
            if !changes.is_empty() {
                return Err(RuntimeError::with_names(
                    ErrorKind::NeedsNewApproval,
                    changes,
                    Some("its connectors changed since it was approved"),
                ));
            }
        }
        Ok(())
    }

    /// Renders as [`render`](Self::render) does, from `data` held in memory as the kept data,
    /// fetched now by a successful update; nothing is read, written or sent. For `runtime
    /// bench`'s `render_with_data_us` (SC-004).
    pub fn render_with_kept(&self, config: &str, data: &serde_json::Value) -> RenderReport {
        let now = calls::now_ms();
        let kept = kept::Kept {
            id: self.manifest().id().to_string(),
            origin: self.origin.clone(),
            digest: self.package.digest.clone(),
            approval: kept::KeptApproval {
                digest: String::new(),
                connectors_on: Vec::new(),
            },
            fetched_at_ms: Some(now),
            data: Some(data.clone()),
            accounts: std::collections::BTreeMap::new(),
            last_update: Some(kept::LastUpdate {
                at_ms: now,
                outcome: kept::Outcome::Ok,
                error: None,
            }),
        };
        let config: serde_json::Value = serde_json::from_str(config)
            .unwrap_or_else(|_| serde_json::Value::String(config.to_owned()));
        let document = kept::render_document(config, Some(&kept)).to_string();
        self.render_document(&document)
    }

    /// Links the plugin again with its grants as they are now, as after a grant change (load
    /// steps 7–9). For `runtime bench` (SC-007).
    pub fn relink(&self) -> Result<(), RuntimeError> {
        let mut linked = self.linked.lock().unwrap_or_else(PoisonError::into_inner);
        linked.generation = self.grants.generation();
        linked.prepared = prepare(
            &self.package,
            &self.origin,
            &self.grants,
            self.options.data_dir(),
        )
        .map(Arc::new);
        linked.prepared.as_ref().map(drop).map_err(Clone::clone)
    }

    /// The plugin linked for the grants as they are now: load steps 7–9 run again whenever the
    /// store's generation moved since the last time.
    fn current(&self) -> Result<Arc<Prepared>, RuntimeError> {
        let mut linked = self.linked.lock().unwrap_or_else(PoisonError::into_inner);
        let generation = self.grants.generation();
        if linked.generation != generation {
            linked.prepared = prepare(
                &self.package,
                &self.origin,
                &self.grants,
                self.options.data_dir(),
            )
            .map(Arc::new);
            linked.generation = generation;
        }
        linked.prepared.clone()
    }

    /// Gives the call its CPU budget and `time_limit`, counted from now.
    fn start_limits(
        &self,
        store: &mut Store<HostState>,
        time_limit: Duration,
    ) -> Result<(), RuntimeError> {
        store
            .set_fuel(self.limits.fuel)
            .map_err(|error| self.call_error(error, time_limit))?;
        // A call must always get its full time. So one tick more than the limit needs, because
        // the current tick has already partly passed, plus the ticks a late ticker still owes:
        // it adds them all at once when it wakes, and they must not come out of this call.
        let ticks = time_limit.as_nanos().div_ceil(EPOCH_TICK.as_nanos())
            + 1
            + u128::from(self.clock.behind());
        store.set_epoch_deadline(u64::try_from(ticks).unwrap_or(u64::MAX));
        Ok(())
    }

    fn call(
        &self,
        prepared: &Prepared,
        store: &mut Store<HostState>,
        config: &str,
    ) -> Result<serde_json::Value, RuntimeError> {
        let rendered = match prepared {
            Prepared::V0(pre) => pre.instantiate(&mut *store).and_then(|instance| {
                instance
                    .dashboard_widget_renderer()
                    .call_render(&mut *store, config)
            }),
            Prepared::V1(v1) => v1.pre.instantiate(&mut *store).and_then(|instance| {
                instance
                    .get_typed_func::<(&str,), (Result<String, String>,)>(&mut *store, &v1.render)?
                    .call(&mut *store, (config,))
                    .map(|(result,)| result)
            }),
        };
        let content = rendered
            .map_err(|error| self.call_error(error, self.limits.time_limit))?
            .map_err(|message| RuntimeError::new(ErrorKind::PluginError, message))?;
        // Before parsing, so oversized content costs no parsing work
        if content.len() > self.limits.max_output_bytes {
            return Err(RuntimeError::new(
                ErrorKind::OutputTooLarge,
                format!(
                    "the content is {} bytes, over the cap of {}",
                    content.len(),
                    self.limits.max_output_bytes
                ),
            ));
        }
        serde_json::from_str(&content)
            .map_err(|error| RuntimeError::new(ErrorKind::InvalidOutput, error.to_string()))
    }

    /// Calls `update` and checks its output: JSON within the update's output limit.
    fn call_update(
        &self,
        prepared: &Prepared,
        store: &mut Store<HostState>,
        update: ComponentExportIndex,
        config: &str,
    ) -> Result<serde_json::Value, RuntimeError> {
        let Prepared::V1(v1) = prepared else {
            unreachable!("only contract v1 has updater")
        };
        let data = v1
            .pre
            .instantiate(&mut *store)
            .and_then(|instance| {
                instance
                    .get_typed_func::<(&str,), (Result<String, String>,)>(&mut *store, &update)?
                    .call(&mut *store, (config,))
                    .map(|(result,)| result)
            })
            .map_err(|error| self.call_error(error, self.limits.update_time_limit))?
            .map_err(|message| RuntimeError::new(ErrorKind::PluginError, message))?;
        if data.len() > self.limits.max_update_output_bytes {
            return Err(RuntimeError::new(
                ErrorKind::OutputTooLarge,
                format!(
                    "the data is {} bytes, over the cap of {}",
                    data.len(),
                    self.limits.max_update_output_bytes
                ),
            ));
        }
        serde_json::from_str(&data)
            .map_err(|error| RuntimeError::new(ErrorKind::InvalidOutput, error.to_string()))
    }

    /// Names the limit that stopped the call, or the trap that crashed it.
    fn call_error(&self, error: wasmtime::Error, time_limit: Duration) -> RuntimeError {
        if let Some(exceeded) = error.downcast_ref::<MemoryLimitExceeded>() {
            return RuntimeError::new(ErrorKind::MemoryLimitExceeded, exceeded.to_string());
        }
        match error.downcast_ref::<Trap>() {
            Some(Trap::OutOfFuel) => RuntimeError::new(
                ErrorKind::CpuBudgetExceeded,
                format!(
                    "used the whole CPU budget of {} fuel units",
                    self.limits.fuel
                ),
            ),
            Some(Trap::Interrupt) if time_limit == self.limits.update_time_limit => {
                RuntimeError::new(
                    ErrorKind::TimeLimitExceeded,
                    format!("still running at the update limit of {time_limit:?}"),
                )
            }
            Some(Trap::Interrupt) => RuntimeError::new(
                ErrorKind::TimeLimitExceeded,
                format!("still running at the time limit of {time_limit:?}"),
            ),
            Some(trap) => RuntimeError::new(ErrorKind::PluginCrashed, trap.to_string()),
            None => RuntimeError::new(ErrorKind::PluginCrashed, format!("{error:#}")),
        }
    }
}

/// Reads at most `max_bytes` of the file at `path`, refusing a larger one.
fn read_capped(path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => "not found".to_owned(),
        _ => error.to_string(),
    })?;
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > max_bytes {
        return Err(format!("larger than the limit of {max_bytes} bytes"));
    }
    Ok(bytes)
}

/// How often the epoch advances, the precision of the time limit.
const EPOCH_TICK: Duration = Duration::from_millis(10);

/// The engine's epoch as a clock: it should advance once per [`EPOCH_TICK`] of real time since
/// `start`, and `ticks` counts the advances the ticker has made.
struct EpochClock {
    start: Instant,
    ticks: AtomicU64,
}

impl EpochClock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            ticks: AtomicU64::new(0),
        }
    }

    /// Advances real time calls for by now.
    fn due(&self) -> u64 {
        let due = self.start.elapsed().as_nanos() / EPOCH_TICK.as_nanos();
        u64::try_from(due).unwrap_or(u64::MAX)
    }

    /// Advances the ticker owes right now, after waking up late.
    fn behind(&self) -> u64 {
        self.due()
            .saturating_sub(self.ticks.load(Ordering::Acquire))
    }
}

/// Advances the engine's epoch on its own thread, following `clock`, for as long as the engine
/// exists.
fn start_epoch_ticker(engine: &Engine, clock: Arc<EpochClock>) -> std::io::Result<()> {
    let engine = engine.weak();
    thread::Builder::new()
        .name("runtime-epoch".to_owned())
        .spawn(move || {
            loop {
                thread::sleep(EPOCH_TICK);
                let Some(engine) = engine.upgrade() else {
                    return;
                };
                // Follows the clock, not the sleeps, so a late wake-up catches up instead of
                // stretching every time limit. The epoch moves before the count, so the count
                // never runs ahead of it.
                let due = clock.due();
                while clock.ticks.load(Ordering::Relaxed) < due {
                    engine.increment_epoch();
                    clock.ticks.fetch_add(1, Ordering::Release);
                }
            }
        })?;
    Ok(())
}
