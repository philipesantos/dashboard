//! The window's reviewed surface (specs/008-tauri-shell/contracts/window-surface.md).
//!
//! `window-surface.toml` lists everything the window may call: each permission granted in
//! `capabilities/`, each app command, each Tauri plugin, with the reason the host UI needs it.
//! This test fails when the list and the app disagree, and when one of the settings that keep
//! the window's protections on is changed.
//!
//! Part of the security core (constitution, Principle IX).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn toml_file(name: &str) -> toml::Table {
    read(&crate_dir().join(name))
        .parse()
        .unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn json_file(path: &Path) -> serde_json::Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The entries of one kind in window-surface.toml: (name, reason).
fn listed(
    surface: &toml::Table,
    kind: &str,
    key: &str,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(entries) = surface.get(kind) else {
        return names;
    };
    for entry in entries.as_array().into_iter().flatten() {
        let name = entry
            .get(key)
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if name.is_empty() {
            failures.push(format!(
                "window-surface.toml: a [[{kind}]] entry has no {key}"
            ));
            continue;
        }
        let reason = entry
            .get("reason")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if reason.trim().is_empty() {
            failures.push(format!(
                "window-surface.toml: entry \"{name}\" has no reason"
            ));
        }
        names.insert(name.to_string());
    }
    names
}

/// The permissions one capability grants, as JSON (a TOML capability is converted first).
fn capability_permissions(
    capability: &serde_json::Value,
    source: &str,
    grants: &mut Vec<(String, String)>,
) {
    for permission in capability["permissions"].as_array().into_iter().flatten() {
        let id = permission
            .as_str()
            .or_else(|| permission["identifier"].as_str())
            .unwrap_or_default();
        grants.push((id.to_string(), source.to_string()));
    }
}

/// The capabilities in one file, in every form Tauri accepts: one capability, a list, or
/// `{ "capabilities": [...] }` (`tauri-utils-2.10.2/src/acl/capability.rs:250-265`).
fn file_capabilities(value: &serde_json::Value) -> Vec<&serde_json::Value> {
    if let Some(list) = value.as_array() {
        list.iter().collect()
    } else if let Some(list) = value["capabilities"].as_array() {
        list.iter().collect()
    } else {
        vec![value]
    }
}

fn capability_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            capability_files(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "json" || ext == "json5" || ext == "toml")
        {
            out.push(path);
        }
    }
}

/// Every permission granted, with where: each file under capabilities/, in any folder and format
/// Tauri reads (`./capabilities/**/*`, JSON or TOML), and each capability written inline in a
/// config file's `app.security.capabilities`.
fn granted(configs: &[(String, serde_json::Value)]) -> Vec<(String, String)> {
    let mut grants = Vec::new();
    let mut files = Vec::new();
    capability_files(&crate_dir().join("capabilities"), &mut files);
    for path in files {
        let source = path
            .strip_prefix(crate_dir())
            .unwrap_or(&path)
            .display()
            .to_string();
        let value = if path.extension().is_some_and(|ext| ext == "toml") {
            let table: toml::Table = read(&path)
                .parse()
                .unwrap_or_else(|error| panic!("{source}: {error}"));
            serde_json::to_value(table).expect("toml to json")
        } else {
            json_file(&path)
        };
        for capability in file_capabilities(&value) {
            capability_permissions(capability, &source, &mut grants);
        }
    }
    for (file, config) in configs {
        for capability in config["app"]["security"]["capabilities"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if capability.is_object() {
                capability_permissions(capability, &format!("{file} (inline)"), &mut grants);
            }
        }
    }
    grants
}

/// tauri.conf.json and the per-system files Tauri merges over it.
fn config_files() -> Vec<(String, serde_json::Value)> {
    [
        "tauri.conf.json",
        "tauri.windows.conf.json",
        "tauri.macos.conf.json",
        "tauri.linux.conf.json",
    ]
    .into_iter()
    .filter(|name| crate_dir().join(name).exists())
    .map(|name| (name.to_string(), json_file(&crate_dir().join(name))))
    .collect()
}

/// Every dependency table of Cargo.toml, target-specific ones included.
fn dependency_tables(manifest: &toml::Table) -> Vec<&toml::Table> {
    let mut tables = Vec::new();
    for key in ["dependencies", "build-dependencies", "dev-dependencies"] {
        if let Some(table) = manifest.get(key).and_then(|value| value.as_table()) {
            tables.push(table);
        }
    }
    if let Some(targets) = manifest.get("target").and_then(|value| value.as_table()) {
        for target in targets.values() {
            for key in ["dependencies", "build-dependencies", "dev-dependencies"] {
                if let Some(table) = target.get(key).and_then(|value| value.as_table()) {
                    tables.push(table);
                }
            }
        }
    }
    tables
}

#[test]
fn the_window_surface_matches_the_reviewed_list() {
    let mut failures = Vec::new();
    let surface = toml_file("window-surface.toml");

    // Permissions: listed == granted
    let configs = config_files();
    let permissions = listed(&surface, "permission", "id", &mut failures);
    let grants = granted(&configs);
    for (id, file) in &grants {
        if !permissions.contains(id) {
            failures.push(format!(
                "window-surface.toml: permission \"{id}\" is granted in {file} but not listed"
            ));
        }
    }
    for id in &permissions {
        if !grants.iter().any(|(granted, _)| granted == id) {
            failures.push(format!(
                "window-surface.toml: permission \"{id}\" is listed but not granted"
            ));
        }
    }

    // Commands: listed == registered (the const the commands! macro expands)
    let commands = listed(&surface, "command", "name", &mut failures);
    let registered: BTreeSet<String> = dashboard_lib::COMMANDS
        .iter()
        .map(|name| name.to_string())
        .collect();
    for name in registered.difference(&commands) {
        failures.push(format!(
            "window-surface.toml: command \"{name}\" is registered but not listed"
        ));
    }
    for name in commands.difference(&registered) {
        failures.push(format!(
            "window-surface.toml: command \"{name}\" is listed but not registered"
        ));
    }

    // Plugins: every tauri-plugin-* dependency is listed, and nothing else
    let plugins = listed(&surface, "plugin", "crate", &mut failures);
    let manifest = toml_file("Cargo.toml");
    let dependencies: BTreeSet<String> = dependency_tables(&manifest)
        .into_iter()
        .flat_map(|table| table.keys())
        .filter(|name| name.starts_with("tauri-plugin-"))
        .cloned()
        .collect();
    for name in dependencies.difference(&plugins) {
        failures.push(format!(
            "window-surface.toml: plugin \"{name}\" is a dependency but not listed"
        ));
    }
    for name in plugins.difference(&dependencies) {
        failures.push(format!(
            "window-surface.toml: plugin \"{name}\" is listed but not a dependency"
        ));
    }

    // The app ACL manifest, for this very build (OUT_DIR is this feature set's build script, so a
    // probe build's manifest cannot stand in for it). Without it, Tauri skips the ACL for app
    // commands (research R4).
    let manifests = json_file(Path::new(concat!(env!("OUT_DIR"), "/acl-manifests.json")));
    if manifests.get("__app-acl__").is_none() {
        failures.push(
            "gen/schemas/acl-manifests.json: no app manifest, so app commands skip the ACL (keep permissions/default.toml)"
                .to_string(),
        );
    }

    // No developer tools compiled into a release build (FR-011)
    for table in dependency_tables(&manifest) {
        for name in ["tauri", "tauri-runtime-wry", "wry"] {
            let features = table
                .get(name)
                .and_then(|dependency| dependency.get("features"))
                .and_then(|features| features.as_array());
            if features
                .into_iter()
                .flatten()
                .any(|feature| feature.as_str() == Some("devtools"))
            {
                failures.push(format!(
                    "Cargo.toml: the release build enables \"devtools\" ({name})"
                ));
            }
        }
    }

    for (file, config) in &configs {
        // No macOS entitlement that lets the system offer passwords for a web domain (research R7)
        if let Some(entitlements) = config["bundle"]["macOS"]["entitlements"].as_str() {
            let plist = read(&crate_dir().join(entitlements));
            for grant in ["com.apple.developer.associated-domains", "webcredentials"] {
                if plist.contains(grant) {
                    failures.push(format!("{file}: the macOS entitlements grant \"{grant}\""));
                }
            }
        }

        // Tauri's drag-and-drop handler stays on: it keeps a dropped file from navigating the
        // window (FR-012). The probe cannot make a real drop, so this is the drop's breaking test.
        for window in config["app"]["windows"].as_array().into_iter().flatten() {
            if window["dragDropEnabled"] == serde_json::Value::Bool(false) {
                failures.push(format!(
                    "{file}: the drag-and-drop handler is turned off (dragDropEnabled: false)"
                ));
            }
        }
    }
    if read(&crate_dir().join("src/window.rs")).contains("disable_drag_drop_handler") {
        failures.push(
            "src/window.rs: the drag-and-drop handler is turned off (disable_drag_drop_handler)"
                .to_string(),
        );
    }

    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}
