//! Plugin runtime: loads plugins packaged as WebAssembly components and runs them in a sandbox,
//! under limits set by the host.

mod error;
mod host;
mod imports;
mod limits;

use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store, Trap};

use host::HostState;

pub use error::{ErrorKind, RuntimeError};
pub use limits::Limits;

// Bindings for the `widget` world of plugin contract v0, the only copy of which is
// wit/widget.wit.
wasmtime::component::bindgen!({ path: "../wit", world: "widget" });

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
    /// Holds only the `log` capability; nothing else can ever be linked into a plugin.
    linker: Linker<HostState>,
    limits: Limits,
}

/// A loaded plugin, compiled once and instantiated afresh for every render call.
pub struct Plugin {
    pre: WidgetPre<HostState>,
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
        let engine = Engine::new(&config)?;

        let mut linker = Linker::new(&engine);
        Widget::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;

        Ok(Runtime {
            engine,
            linker,
            limits,
        })
    }

    /// Checks and compiles a plugin package. A failure stops at the first step that refuses
    /// the package, with that step's error kind.
    pub fn load(&self, bytes: &[u8]) -> Result<Plugin, RuntimeError> {
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
        // from_binary never accepts the text format, whatever Wasmtime features are enabled.
        let component = Component::from_binary(&self.engine, bytes)
            .map_err(|error| RuntimeError::new(ErrorKind::InvalidPackage, format!("{error:#}")))?;
        // Before linking, so every refused import is named, not just the first the linker misses.
        imports::check(&self.engine, &component)?;

        let mismatch = |error: wasmtime::Error| {
            RuntimeError::new(ErrorKind::ContractMismatch, format!("{error:#}"))
        };
        let pre = self.linker.instantiate_pre(&component).map_err(mismatch)?;
        let pre = WidgetPre::new(pre).map_err(mismatch)?;
        Ok(Plugin { pre })
    }
}

impl Plugin {
    /// Calls the plugin's `render` with a JSON configuration, in a fresh instance that is
    /// dropped when the call ends, so nothing survives from one call to the next (FR-008).
    pub fn render(&self, config: &str) -> RenderReport {
        let mut store = Store::new(self.pre.engine(), HostState::default());
        let result = self.call(&mut store, config);
        let state = store.into_data();
        RenderReport {
            result,
            logs: state.logs,
            dropped_logs: 0,
        }
    }

    fn call(
        &self,
        store: &mut Store<HostState>,
        config: &str,
    ) -> Result<serde_json::Value, RuntimeError> {
        let instance = self.pre.instantiate(&mut *store).map_err(crashed)?;
        let content = instance
            .dashboard_widget_renderer()
            .call_render(&mut *store, config)
            .map_err(crashed)?
            .map_err(|message| RuntimeError::new(ErrorKind::PluginError, message))?;
        serde_json::from_str(&content)
            .map_err(|error| RuntimeError::new(ErrorKind::InvalidOutput, error.to_string()))
    }
}

/// A failure inside the plugin that no limit explains, named by its trap when there is one.
fn crashed(error: wasmtime::Error) -> RuntimeError {
    let detail = match error.downcast_ref::<Trap>() {
        Some(trap) => trap.to_string(),
        None => format!("{error:#}"),
    };
    RuntimeError::new(ErrorKind::PluginCrashed, detail)
}
