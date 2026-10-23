//! Plugin runtime: loads plugins packaged as WebAssembly components and runs them in a sandbox,
//! under limits set by the host, with exactly the capabilities their manifest declares and the
//! user granted (`specs/002-plugin-permissions/`).

mod error;
pub mod grants;
mod host;
mod limits;
pub mod manifest;
pub mod package;
mod permissions;
mod text;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store, Trap};

use grants::GrantStore;
use host::HostState;
use limits::MemoryLimitExceeded;
use manifest::Manifest;
use permissions::Interfaces;

pub use error::{ErrorKind, RuntimeError};
pub use limits::Limits;
pub use permissions::Contract;
pub use text::escape;

/// Bindings for the `widget` world of plugin contract v0 (`wit/v0`, `dashboard:widget@0.1.0`).
pub(crate) mod v0 {
    wasmtime::component::bindgen!({ path: "../wit/v0", world: "widget" });
}

/// Bindings for the `widget` world of plugin contract v1 (`wit/v1`, `dashboard:widget@1.0.0`).
pub(crate) mod v1 {
    wasmtime::component::bindgen!({ path: "../wit/v1", world: "widget" });
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

/// Loads plugins and runs them under the host's limits.
pub struct Runtime {
    engine: Engine,
    limits: Limits,
    clock: Arc<EpochClock>,
}

/// A package that passed every check that needs no grants: load steps 1–6 of
/// `specs/002-plugin-permissions/data-model.md`.
pub struct CheckedPackage {
    component: Component,
    interfaces: Interfaces,
    manifest: Manifest,
    contract: Contract,
}

impl CheckedPackage {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn contract(&self) -> Contract {
        self.contract
    }
}

/// A loaded plugin, compiled once and instantiated afresh for every render call.
pub struct Plugin {
    package: CheckedPackage,
    /// Linked with the capabilities the plugin's approval from this origin grants.
    prepared: Prepared,
    limits: Limits,
    clock: Arc<EpochClock>,
}

/// A plugin linked for its contract version, ready to instantiate.
enum Prepared {
    V0(v0::WidgetPre<HostState>),
    V1(v1::WidgetPre<HostState>),
}

impl Runtime {
    /// Starts Wasmtime with every WebAssembly proposal the runtime does not need turned off.
    ///
    /// Fails only if Wasmtime cannot start on this machine.
    pub fn new(limits: Limits) -> wasmtime::Result<Runtime> {
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
        let prepared = prepare(&package, origin, &grants)?;
        Ok(Plugin {
            package,
            prepared,
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
        package.contract,
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
        Contract::V1 => Prepared::V1(v1::WidgetPre::new(pre).map_err(mismatch)?),
    })
}

impl Plugin {
    /// The manifest the plugin was loaded with.
    pub fn manifest(&self) -> &Manifest {
        &self.package.manifest
    }

    /// The contract version the plugin implements.
    pub fn contract(&self) -> Contract {
        self.package.contract
    }

    /// Calls the plugin's `render` with a JSON configuration, in a fresh instance that is
    /// dropped when the call ends, so nothing survives from one call to the next (FR-008).
    pub fn render(&self, config: &str) -> RenderReport {
        let mut store = Store::new(
            self.package.component.engine(),
            HostState::new(&self.limits),
        );
        store.limiter(|state| &mut state.limiter);
        let result = self
            .start_limits(&mut store)
            .and_then(|()| self.call(&mut store, config));
        let state = store.into_data();
        RenderReport {
            result,
            logs: state.logs,
            dropped_logs: state.dropped_logs,
        }
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
        store: &mut Store<HostState>,
        config: &str,
    ) -> Result<serde_json::Value, RuntimeError> {
        let rendered = match &self.prepared {
            Prepared::V0(pre) => pre.instantiate(&mut *store).and_then(|instance| {
                instance
                    .dashboard_widget_renderer()
                    .call_render(&mut *store, config)
            }),
            Prepared::V1(pre) => pre.instantiate(&mut *store).and_then(|instance| {
                instance
                    .dashboard_widget_renderer()
                    .call_render(&mut *store, config)
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
