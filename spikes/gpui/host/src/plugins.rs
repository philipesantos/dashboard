//! The host side of plugins, through the `runtime` crate unchanged: the packages this spike
//! offers, their approval, loading and render calls.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use runtime::grants::GrantStore;
use runtime::{ErrorKind, Limits, Plugin, Runtime, RuntimeOptions};
use serde_json::{Value, json};

/// A package built into the spike: the repository's hello-widget and the spike's sampler.
pub struct Package {
    pub bytes: &'static [u8],
    /// The origin approvals are bound to (a path in the CLI; here, the embedded package).
    pub origin: &'static str,
}

pub const HELLO: Package = Package {
    bytes: include_bytes!("../../../../target/plugins/hello-widget.wasm"),
    origin: "spike-embedded:plugins/examples/hello-widget",
};

pub const SAMPLER: Package = Package {
    bytes: include_bytes!("../../target/plugins/ui-sampler.wasm"),
    origin: "spike-embedded:spikes/gpui/plugin/ui-sampler",
};

/// What the toolbar can add: a package with a configuration and a starting size in cells.
pub struct Entry {
    pub key: &'static str,
    pub name: &'static str,
    pub package: &'static Package,
    pub config: fn(clicks: u64) -> Value,
    pub size: (i32, i32),
}

pub const CATALOG: &[Entry] = &[
    Entry {
        key: "hello",
        name: "Hello",
        package: &HELLO,
        config: |_| json!({"name": "GPUI"}),
        size: (3, 2),
    },
    Entry {
        key: "sampler",
        name: "Release status",
        package: &SAMPLER,
        config: |clicks| json!({"mode": "sampler", "clicks": clicks}),
        size: (5, 5),
    },
    Entry {
        key: "overflow",
        name: "Overflow test",
        package: &SAMPLER,
        config: |_| json!({"mode": "overflow"}),
        size: (4, 3),
    },
    Entry {
        key: "spoof",
        name: "Spoof test",
        package: &SAMPLER,
        config: |_| json!({"mode": "spoof"}),
        size: (4, 3),
    },
    Entry {
        key: "password",
        name: "Password test",
        package: &SAMPLER,
        config: |_| json!({"mode": "password"}),
        size: (3, 2),
    },
    Entry {
        key: "host-color",
        name: "Host color test",
        package: &SAMPLER,
        config: |_| json!({"mode": "host-color"}),
        size: (3, 2),
    },
];

pub fn entry(key: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|entry| entry.key == key)
}

/// What the approval dialog shows about a package.
#[derive(Debug, Clone)]
pub struct Request {
    pub id: String,
    pub version: String,
    pub author: String,
    pub contract: String,
    /// Each permission with whether it is required.
    pub permissions: Vec<(String, bool)>,
}

pub enum Loaded {
    Ready(Arc<Plugin>),
    /// No approval for this package yet, or it asks for more than was approved.
    NeedsApproval(Request),
    Refused(String),
}

pub struct PluginHost {
    runtime: Runtime,
    grants: Arc<GrantStore>,
    loaded: HashMap<&'static str, Arc<Plugin>>,
}

impl PluginHost {
    pub fn new(data_dir: &Path) -> anyhow::Result<PluginHost> {
        let runtime = Runtime::new(
            Limits::default(),
            RuntimeOptions::new(data_dir.join("runtime")),
        )
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let grants = Arc::new(GrantStore::open(data_dir.join("grants.json")));
        Ok(PluginHost {
            runtime,
            grants,
            loaded: HashMap::new(),
        })
    }

    pub fn load(&mut self, package: &'static Package) -> Loaded {
        if let Some(plugin) = self.loaded.get(package.origin) {
            return Loaded::Ready(plugin.clone());
        }
        match self
            .runtime
            .load(package.bytes, package.origin, self.grants.clone())
        {
            Ok(plugin) => {
                let plugin = Arc::new(plugin);
                self.loaded.insert(package.origin, plugin.clone());
                Loaded::Ready(plugin)
            }
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::NotGranted | ErrorKind::NeedsNewApproval
                ) =>
            {
                match self.runtime.check(package.bytes) {
                    Ok(checked) => {
                        let manifest = checked.manifest();
                        Loaded::NeedsApproval(Request {
                            id: manifest.id().as_str().to_owned(),
                            version: manifest.version().to_owned(),
                            author: manifest.author().to_owned(),
                            contract: checked.contract_version().display(),
                            permissions: manifest
                                .permissions()
                                .iter()
                                .map(|p| (p.name.clone(), p.required))
                                .collect(),
                        })
                    }
                    Err(error) => Loaded::Refused(error.to_string()),
                }
            }
            Err(error) => Loaded::Refused(error.to_string()),
        }
    }

    /// Approves `package`'s required permissions. Called only from the host's approval dialog.
    pub fn approve(&mut self, package: &'static Package) -> Result<(), String> {
        let checked = self
            .runtime
            .check(package.bytes)
            .map_err(|error| error.to_string())?;
        checked
            .approve(&self.grants, package.origin, None)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// Renders off the main thread. The runtime's limits (fuel, time, memory) bound the call.
pub fn render(plugin: &Plugin, config: &Value) -> Result<widget_kit::model::Content, String> {
    let report = plugin.render(&config.to_string());
    let value = report.result.map_err(|error| error.to_string())?;
    widget_kit::model::Content::parse(&value).map_err(|error| error.to_string())
}

/// Where the spike keeps its own records: never the app's or the development CLI's.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SPIKE_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("dev.philsantos.dashboard-gpui-spike")
}
