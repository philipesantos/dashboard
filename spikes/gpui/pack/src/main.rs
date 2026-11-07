//! Builds spikes/gpui/plugin/ui-sampler for wasm32-unknown-unknown, makes it a component and
//! appends its manifest, as `cargo xtask plugins` does for plugins/ (xtask/src/main.rs). The
//! package lands in spikes/gpui/target/plugins/ui-sampler.wasm, where the host embeds it.

use std::borrow::Cow;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::{env, fs};

use wasm_encoder::{ComponentSection, CustomSection};
use wit_component::ComponentEncoder;

/// The custom section the runtime reads the manifest from (runtime/src/package.rs).
const MANIFEST_SECTION: &str = "dashboard-manifest";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let spike = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("pack has no parent directory")?;
    let plugin = spike.join("plugin");
    let build_dir = spike.join("target").join("plugin-build");
    let out_dir = spike.join("target").join("plugins");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(plugin.join("Cargo.toml"))
        .args([
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--target-dir",
        ])
        .arg(&build_dir)
        .args(env::args().skip(1)) // e.g. --locked
        .status()?;
    if !status.success() {
        return Err("building ui-sampler failed".into());
    }
    let module = fs::read(
        build_dir
            .join("wasm32-unknown-unknown")
            .join("release")
            .join("ui_sampler.wasm"),
    )?;
    let mut component = ComponentEncoder::default()
        .module(&module)?
        .validate(true)
        .encode()?;
    let manifest = fs::read(plugin.join("ui-sampler").join("manifest.json"))?;
    CustomSection {
        name: Cow::Borrowed(MANIFEST_SECTION),
        data: Cow::Owned(manifest),
    }
    .append_to_component(&mut component);
    fs::create_dir_all(&out_dir)?;
    let path = out_dir.join("ui-sampler.wasm");
    fs::write(&path, component)?;
    println!("{}", path.display());
    Ok(())
}
