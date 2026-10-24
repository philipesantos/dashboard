//! Plugin runtime: loads plugins packaged as WebAssembly components and runs them in a sandbox,
//! under limits set by the host, with exactly the capabilities their manifest declares and the
//! user granted (`specs/002-plugin-permissions/`), and reach external services only through
//! the connectors they declare, which the host runs (`specs/003-plugin-connectors/`).

// The test seam replaces DNS and the trusted roots; it must never reach a build anyone runs
// (specs/003-plugin-connectors/research.md, R15).
#[cfg(all(feature = "test-network", not(debug_assertions)))]
compile_error!(
    "the test-network feature is only for the runtime's own tests, which run in debug builds: a release build never includes the test seam"
);

// Unused until loading runs connectors (specs/003-plugin-connectors/tasks.md, T035)
#[allow(dead_code)]
mod calls;
// Unused until loading runs connectors (specs/003-plugin-connectors/tasks.md, T035)
#[allow(dead_code)]
mod catalog;
// Unused until loading runs connectors (specs/003-plugin-connectors/tasks.md, T035)
#[allow(dead_code)]
mod connector;
mod error;
pub mod grants;
mod host;
// Unused until loading runs connectors (specs/003-plugin-connectors/tasks.md, T035)
#[allow(dead_code)]
mod kept;
mod limits;
pub mod manifest;
// Unused until loading runs connectors (specs/003-plugin-connectors/tasks.md, T035)
#[allow(dead_code)]
mod network;
pub mod package;
mod permissions;
// Unused until loading runs connectors (specs/003-plugin-connectors/tasks.md, T035)
#[allow(dead_code)]
mod requests;
mod text;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use wasmtime::component::{Component, ComponentExportIndex, InstancePre, Linker};
use wasmtime::{Config, Engine, Store, Trap};

use grants::GrantStore;
use host::HostState;
use limits::MemoryLimitExceeded;
use manifest::Manifest;
use permissions::Interfaces;

pub use error::{ErrorKind, RuntimeError};
pub use limits::Limits;
#[cfg(feature = "test-network")]
pub use network::{Route, TestNetwork};
pub use permissions::{Contract, ContractVersion};
pub use text::escape;

/// Bindings for the `widget` world of plugin contract v0 (`wit/v0`, `dashboard:widget@0.1.0`).
pub(crate) mod v0 {
    wasmtime::component::bindgen!({ path: "../wit/v0", world: "widget" });
}

/// Bindings for plugin contract v1 (`wit/v1`, `dashboard:widget@1.1.0`), from its widest world,
/// `connected-widget`, for the host side of every interface. Wasmtime matches the imports of
/// plugins built for an older minor against them by compatible version. Exports are looked up
/// one by one, by the exact names each plugin has, since a render-only widget has no `updater`
/// (`specs/003-plugin-connectors/research.md`, R11).
pub(crate) mod v1 {
    wasmtime::component::bindgen!({ path: "../wit/v1", world: "connected-widget" });
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
    // Read once loading runs connectors (specs/003-plugin-connectors/tasks.md, T035)
    #[allow(dead_code)]
    options: RuntimeOptions,
    #[allow(dead_code)]
    catalog: catalog::Catalog,
    #[allow(dead_code)]
    network: network::Network,
}

/// A package that passed every check that needs no grants: load steps 1–6 of
/// `specs/002-plugin-permissions/data-model.md`.
pub struct CheckedPackage {
    component: Component,
    interfaces: Interfaces,
    manifest: Manifest,
    contract: ContractVersion,
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
    /// The `update` function, for a plugin that exports `updater`; looked up once loading runs
    /// updates (specs/003-plugin-connectors/tasks.md, T035).
    #[allow(dead_code)]
    update: Option<ComponentExportIndex>,
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
        Runtime::start(limits, options, catalog, network::Network::with_seam(test))
    }

    fn start(
        limits: Limits,
        options: RuntimeOptions,
        catalog: catalog::Catalog,
        network: network::Network,
    ) -> wasmtime::Result<Runtime> {
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

        Ok(Runtime {
            engine,
            limits,
            clock,
            options,
            catalog,
            network,
        })
    }

    /// Checks a plugin package without its grants: size, manifest, compilation, contract and
    /// imports (load steps 1–6). A failure stops at the first step that refuses the package,
    /// with that step's error kind.
    pub fn check(&self, bytes: &[u8]) -> Result<CheckedPackage, RuntimeError> {
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
        // Step 4: from_binary never accepts the text format, whatever Wasmtime features are
        // enabled.
        let component = Component::from_binary(&self.engine, bytes)
            .map_err(|error| RuntimeError::new(ErrorKind::InvalidPackage, format!("{error:#}")))?;
        // Steps 5 and 6: on names, before linking, so every refused import is named
        let interfaces = Interfaces::of(&self.engine, &component);
        let contract = permissions::check_component(&interfaces, &manifest)?;
        Ok(CheckedPackage {
            component,
            interfaces,
            manifest,
            contract,
        })
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
        let prepared = prepare(&package, origin, &grants)?;
        Ok(Plugin {
            package,
            origin: origin.to_owned(),
            grants,
            linked: Mutex::new(Linked {
                generation,
                prepared: Ok(Arc::new(prepared)),
            }),
            limits: self.limits.clone(),
            clock: Arc::clone(&self.clock),
        })
    }
}

/// Load steps 7–9: the plugin's approval from `origin`, pruning, then a linker built for this
/// plugin alone, holding the real interface of each granted import and the stand-in of each
/// optional one that is off. Nothing else is ever linked (Principle II).
fn prepare(
    package: &CheckedPackage,
    origin: &str,
    grants: &GrantStore,
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
    if !decision.prune.is_empty() {
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
                };
            }
        };
        let mut store = Store::new(
            self.package.component.engine(),
            HostState::new(&self.limits),
        );
        store.limiter(|state| &mut state.limiter);
        let result = self
            .start_limits(&mut store)
            .and_then(|()| self.call(&prepared, &mut store, config));
        let state = store.into_data();
        RenderReport {
            result,
            logs: state.logs,
            dropped_logs: state.dropped_logs,
        }
    }

    /// Runs again every check of a load but the compile and the linking: the manifest section
    /// and the manifest, the contract and the imports, and the grants (load steps 2, 3 and
    /// 5–8), on the package bytes it was loaded from. For `runtime bench` (SC-007).
    pub fn recheck(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        let manifest = Manifest::parse(package::manifest_section(bytes)?)?;
        let contract = permissions::check_component(&self.package.interfaces, &manifest)?;
        let entry = self.grants.entry(manifest.id())?;
        permissions::decide(
            contract.contract,
            &self.package.interfaces,
            &manifest,
            &self.origin,
            entry.as_ref(),
        )
        .map(drop)
    }

    /// Links the plugin again with its grants as they are now, as after a grant change (load
    /// steps 7–9). For `runtime bench` (SC-007).
    pub fn relink(&self) -> Result<(), RuntimeError> {
        let mut linked = self.linked.lock().unwrap_or_else(PoisonError::into_inner);
        linked.generation = self.grants.generation();
        linked.prepared = prepare(&self.package, &self.origin, &self.grants).map(Arc::new);
        linked.prepared.as_ref().map(drop).map_err(Clone::clone)
    }

    /// The plugin linked for the grants as they are now: load steps 7–9 run again whenever the
    /// store's generation moved since the last time.
    fn current(&self) -> Result<Arc<Prepared>, RuntimeError> {
        let mut linked = self.linked.lock().unwrap_or_else(PoisonError::into_inner);
        let generation = self.grants.generation();
        if linked.generation != generation {
            linked.prepared = prepare(&self.package, &self.origin, &self.grants).map(Arc::new);
            linked.generation = generation;
        }
        linked.prepared.clone()
    }

    /// Gives the call its CPU budget and its time limit, counted from now.
    fn start_limits(&self, store: &mut Store<HostState>) -> Result<(), RuntimeError> {
        store
            .set_fuel(self.limits.fuel)
            .map_err(|error| self.call_error(error))?;
        // A call must always get its full time. So one tick more than the limit needs, because
        // the current tick has already partly passed, plus the ticks a late ticker still owes:
        // it adds them all at once when it wakes, and they must not come out of this call.
        let ticks = self
            .limits
            .time_limit
            .as_nanos()
            .div_ceil(EPOCH_TICK.as_nanos())
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
            .map_err(|error| self.call_error(error))?
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

    /// Names the limit that stopped the call, or the trap that crashed it.
    fn call_error(&self, error: wasmtime::Error) -> RuntimeError {
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
            Some(Trap::Interrupt) => RuntimeError::new(
                ErrorKind::TimeLimitExceeded,
                format!(
                    "still running at the time limit of {:?}",
                    self.limits.time_limit
                ),
            ),
            Some(trap) => RuntimeError::new(ErrorKind::PluginCrashed, trap.to_string()),
            None => RuntimeError::new(ErrorKind::PluginCrashed, format!("{error:#}")),
        }
    }
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
