//! Repository tasks that must run the same way on Windows, macOS and Linux.
//!
//! `cargo xtask plugins [--locked]` builds every crate of the `plugins/` workspace and writes
//! each one, as a WebAssembly component, to `target/plugins/<crate-name>.wasm`.
//!
//! Each component ends with the crate's `manifest.json`, the file beside its `Cargo.toml`, as a
//! custom section named `dashboard-manifest` (`specs/002-plugin-permissions/contracts/package.md`).
//!
//! `cargo xtask clean-space` frees the disk that builds and tests leave behind, without forcing a
//! full rebuild (`clean_space`).
//! A crate's `Cargo.toml` can set two keys under `[package.metadata.dashboard]`:
//!
//! - `target`: `"wasm32-unknown-unknown"` (the default) or `"wasm32-wasip2"`, for hostile
//!   plugins that must import WASI the way the standard library does.
//! - `manifest = false`: write the component bare, without a manifest section, for test
//!   fixtures that the tests pack with a manifest of their own. A crate without this key must
//!   have a `manifest.json`.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::{env, fs};

use serde_json::Value;
use wasm_encoder::{ComponentSection, CustomSection};
use wit_component::ComponentEncoder;

mod clean_space;
mod padding;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The default target: plain WebAssembly, which imports only what the contract's WIT declares.
const UNKNOWN: &str = "wasm32-unknown-unknown";
/// Opt-in, for hostile plugins that must import WASI the way the standard library does.
const WASIP2: &str = "wasm32-wasip2";

/// The custom section that carries a plugin's manifest.
const MANIFEST_SECTION: &str = "dashboard-manifest";
/// The manifest's file, beside the crate's `Cargo.toml`.
const MANIFEST_FILE: &str = "manifest.json";

/// A crate of the `plugins/` workspace.
struct PluginCrate {
    name: String,
    target: String,
    /// The crate's directory, where its `manifest.json` lives.
    dir: PathBuf,
    /// False when the crate opts out with `manifest = false`.
    manifest: bool,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["plugins"] => plugins(false),
        ["plugins", "--locked"] => plugins(true),
        ["clean-space"] => root().and_then(clean_space::run),
        _ => {
            eprintln!("usage: cargo xtask plugins [--locked] | cargo xtask clean-space");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The repository's root, the parent of the xtask crate.
fn root() -> Result<&'static Path> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("the xtask crate has no parent directory")?)
}

fn plugins(locked: bool) -> Result<()> {
    let root = root()?;
    let manifest = root.join("plugins").join("Cargo.toml");
    let build_dir = root.join("target").join("plugins-build");
    let out_dir = root.join("target").join("plugins");

    let crates = plugin_crates(&manifest)?;
    if crates.is_empty() {
        println!("no crates in {}; nothing to build", manifest.display());
        return Ok(());
    }

    // Start empty, so a plugin removed from the workspace leaves no stale component behind.
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir)?;
    }
    fs::create_dir_all(&out_dir)?;

    let mut by_target: BTreeMap<&str, Vec<&PluginCrate>> = BTreeMap::new();
    for plugin in &crates {
        by_target.entry(&plugin.target).or_default().push(plugin);
    }

    // Each crate's component before its manifest is appended, for the install fixtures
    let mut bare: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (target, plugins) in by_target {
        let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name.as_str()).collect();
        let mut build = cargo();
        build
            .current_dir(root)
            .arg("build")
            .arg("--manifest-path")
            .arg(&manifest)
            .args(["--release", "--target", target])
            .arg("--target-dir")
            .arg(&build_dir);
        for name in &names {
            build.args(["--package", name]);
        }
        if locked {
            build.arg("--locked");
        }
        let status = build.status()?;
        if !status.success() {
            return Err(format!("building {} for {target} failed", names.join(", ")).into());
        }

        for plugin in plugins {
            let name = &plugin.name;
            // Cargo names the artifact after the crate, with underscores instead of dashes.
            let artifact = build_dir
                .join(target)
                .join("release")
                .join(format!("{}.wasm", name.replace('-', "_")));
            let bytes = fs::read(&artifact)
                .map_err(|error| format!("reading {}: {error}", artifact.display()))?;
            let mut component = if target == UNKNOWN {
                ComponentEncoder::default()
                    .module(&bytes)?
                    .validate(true)
                    .encode()?
            } else {
                // wasm32-wasip2 already produces a component.
                bytes
            };
            bare.insert(name.clone(), component.clone());
            if plugin.manifest {
                append_manifest(&mut component, &plugin.dir.join(MANIFEST_FILE))?;
            }
            let path = out_dir.join(format!("{name}.wasm"));
            fs::write(&path, component)?;
            println!("{}", path.display());
        }
    }
    install_fixtures(root, &bare, &out_dir)
}

/// The largest package the host accepts (`runtime/src/limits.rs`, `max_package_bytes`).
const PACKAGE_CAP: usize = 10 * 1024 * 1024;

/// Packs the install fixtures listed in `plugins/fixtures/install/index.json`: each one is a
/// crate's component with that fixture's manifest, written to `target/plugins/<name>.wasm`
/// (`specs/010-plugin-installation/tasks.md`, T005, T006). With `"padding": "many"`, a core
/// module of generated code that is never instantiated is added before the manifest, so the
/// package is just under the package cap and costly to compile, while the widget renders as the
/// crate does.
fn install_fixtures(root: &Path, bare: &BTreeMap<String, Vec<u8>>, out_dir: &Path) -> Result<()> {
    let dir = root.join("plugins").join("fixtures").join("install");
    let index: Value = serde_json::from_slice(&fs::read(dir.join("index.json"))?)?;
    let fixtures = index["fixtures"]
        .as_array()
        .ok_or("plugins/fixtures/install/index.json has no \"fixtures\" list")?;
    for fixture in fixtures {
        let field = |key: &str| {
            fixture[key]
                .as_str()
                .ok_or_else(|| format!("an install fixture has no \"{key}\""))
        };
        let (name, crate_name, manifest) =
            (field("name")?, field("component")?, field("manifest")?);
        let component = bare
            .get(crate_name)
            .ok_or_else(|| format!("{name}: no crate named {crate_name} was built"))?;
        let mut package = component.clone();
        if let Some(shape) = fixture.get("padding") {
            if shape != "many" {
                return Err(format!("{name}: \"padding\" must be \"many\", found {shape}").into());
            }
            package = padded(component, &dir.join(manifest))?;
        } else {
            append_manifest(&mut package, &dir.join(manifest))?;
        }
        let path = out_dir.join(format!("{name}.wasm"));
        fs::write(&path, package)?;
        println!("{}", path.display());
    }
    Ok(())
}

/// `component` with a module of generated code and the manifest, as large as fits under the
/// package cap with 64 KiB to spare.
fn padded(component: &[u8], manifest: &Path) -> Result<Vec<u8>> {
    let mut code_bytes = PACKAGE_CAP - component.len() - 512 * 1024;
    loop {
        let module = wat::parse_str(padding::many(code_bytes))?;
        let mut package = component.to_vec();
        wasm_encoder::RawSection {
            id: wasm_encoder::ComponentSectionId::CoreModule.into(),
            data: &module,
        }
        .append_to_component(&mut package);
        append_manifest(&mut package, manifest)?;
        if package.len() <= PACKAGE_CAP - 64 * 1024 {
            return Ok(package);
        }
        code_bytes -= 256 * 1024;
    }
}

/// Appends the manifest file's exact bytes to `component` as its manifest section. The runtime
/// checks the manifest when it loads the package.
fn append_manifest(component: &mut Vec<u8>, path: &Path) -> Result<()> {
    let data = fs::read(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
    CustomSection {
        name: Cow::Borrowed(MANIFEST_SECTION),
        data: Cow::Owned(data),
    }
    .append_to_component(component);
    Ok(())
}

/// Lists the workspace members of `plugins/` with the target each one builds for, its
/// directory and whether it gets a manifest section.
fn plugin_crates(manifest: &Path) -> Result<Vec<PluginCrate>> {
    let output = cargo()
        .arg("metadata")
        .arg("--manifest-path")
        .arg(manifest)
        .args(["--format-version", "1", "--no-deps"])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata returned no package list")?;

    let mut crates = Vec::new();
    for package in packages {
        let name = package["name"]
            .as_str()
            .ok_or("cargo metadata returned a package without a name")?;
        // Declared in the crate's Cargo.toml as [package.metadata.dashboard] target = "…"
        let target = match &package["metadata"]["dashboard"]["target"] {
            Value::Null => UNKNOWN,
            Value::String(target) if target == UNKNOWN || target == WASIP2 => target,
            other => {
                return Err(format!(
                    "{name}: [package.metadata.dashboard] target must be \"{UNKNOWN}\" or \"{WASIP2}\", found {other}"
                )
                .into());
            }
        };
        // Declared as [package.metadata.dashboard] manifest = false
        let has_manifest = match &package["metadata"]["dashboard"]["manifest"] {
            Value::Null => true,
            Value::Bool(false) => false,
            other => {
                return Err(format!(
                    "{name}: [package.metadata.dashboard] manifest can only be false, found {other}"
                )
                .into());
            }
        };
        let dir = package["manifest_path"]
            .as_str()
            .map(Path::new)
            .and_then(Path::parent)
            .ok_or_else(|| format!("cargo metadata returned no manifest path for {name}"))?
            .to_path_buf();
        if has_manifest && !dir.join(MANIFEST_FILE).is_file() {
            return Err(format!(
                "{name}: no {MANIFEST_FILE} beside its Cargo.toml; add one, or set [package.metadata.dashboard] manifest = false"
            )
            .into());
        }
        crates.push(PluginCrate {
            name: name.to_owned(),
            target: target.to_owned(),
            dir,
            manifest: has_manifest,
        });
    }
    Ok(crates)
}

/// The cargo that runs this task, so plugins build with the same toolchain.
fn cargo() -> Command {
    Command::new(env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from))
}
