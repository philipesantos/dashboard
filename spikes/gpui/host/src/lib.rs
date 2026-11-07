//! GPUI spike host (spikes/gpui/README.md): a dashboard whose widgets are plugins run by the
//! `runtime` crate, drawn by the host's widget kit inside host frames.

pub mod dashboard;
pub mod layout;
pub mod plugins;
pub mod theme;

use std::path::Path;

/// `--dev-approve`: approves the spike's two embedded packages and lays out four widgets, the
/// way the development CLI grants to local packages (constitution, Principle VII), for
/// unattended measurement. The record it writes is the spike's own, in `data_dir`.
pub fn dev_approve(data_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let mut host = plugins::PluginHost::new(data_dir)?;
    for package in [&plugins::HELLO, &plugins::SAMPLER] {
        host.approve(package).map_err(anyhow::Error::msg)?;
    }
    let mut layout = layout::Layout::default();
    for key in ["sampler", "hello", "overflow", "spoof"] {
        let entry = plugins::entry(key).expect("catalog entry");
        layout.add(entry.key, entry.size.0, entry.size.1);
    }
    std::fs::write(
        data_dir.join("layout.json"),
        serde_json::to_vec_pretty(&layout)?,
    )?;
    if !data_dir.join("settings.json").exists() {
        std::fs::write(
            data_dir.join("settings.json"),
            br#"{"mode":"System","secret":"Teal"}"#,
        )?;
    }
    Ok(())
}
