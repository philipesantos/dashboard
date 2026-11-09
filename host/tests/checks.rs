//! Checks that fail the build when a rule of the host is broken in its sources or manifests
//! (specs/008-host-window-dialogs/contracts/bans-and-checks.md).

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace")
        .to_path_buf()
}

fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// Every `.rs` file under `host/src/`, with its path from the repository root.
fn host_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("a directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root().join("host/src"), &mut files);
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(root())
                .expect("inside the repository")
                .to_string_lossy()
                .replace('\\', "/");
            let text = fs::read_to_string(&path).expect("readable");
            (name, text)
        })
        .collect()
}

/// The source without `//` comments, so a rule's text in a comment is not a use.
fn code(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) if !line[..at].contains('"') => &line[..at],
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The version `host/README.md` records for GPUI.
fn recorded_gpui_version() -> String {
    let readme = read("host/README.md");
    let line = readme
        .lines()
        .find(|line| line.starts_with("| Version |"))
        .expect("host/README.md records the GPUI version");
    line.split('`')
        .nth(1)
        .expect("the version in backticks")
        .to_owned()
}

/// The gpui-unofficial packages `host/Cargo.toml` depends on, with their requirement.
fn host_gpui_dependencies() -> Vec<(String, String)> {
    read("host/Cargo.toml")
        .lines()
        .filter(|line| !line.trim_start().starts_with('#') && line.contains("gpui-unofficial"))
        .map(|line| {
            let (key, value) = line.split_once('=').expect("a dependency line");
            let name = match value.split_once("package = \"") {
                Some((_, rest)) => rest.split('"').next().expect("a name").to_owned(),
                None => key.trim().to_owned(),
            };
            let version = value
                .split('"')
                .find(|part| {
                    part.starts_with('=')
                        || part
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_digit() || c == '^' || c == '~')
                })
                .or_else(|| {
                    value
                        .split_once("version = \"")
                        .map(|(_, rest)| rest.split('"').next().expect("a version"))
                })
                .expect("a version requirement")
                .to_owned();
            (name, version)
        })
        .collect()
}

/// `(name, version)` of every package in `Cargo.lock` whose name holds `gpui-unofficial`.
fn locked_gpui_packages() -> Vec<(String, String)> {
    let lock = read("Cargo.lock");
    lock.split("[[package]]")
        .filter_map(|block| {
            let field = |key: &str| {
                block
                    .lines()
                    .find_map(|line| line.strip_prefix(&format!("{key} = \"")))
                    .map(|rest| rest.trim_end_matches('"').to_owned())
            };
            let name = field("name")?;
            name.contains("gpui-unofficial")
                .then(|| (name, field("version").expect("a version")))
        })
        .collect()
}

/// R1, FR-003: every gpui-unofficial package is a direct `=` dependency of `host`, at the version
/// `host/README.md` records, never a pre-release.
#[test]
fn gpui_is_pinned_as_recorded() {
    let recorded = recorded_gpui_version();
    assert!(
        !recorded.contains('-'),
        "the recorded version {recorded} is a pre-release"
    );
    // The dev-dependency with GPUI's test support names gpui-unofficial again, pinned the same
    let direct = host_gpui_dependencies();
    let mut names: Vec<&str> = direct.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(),
        19,
        "nineteen gpui-unofficial crates: {names:?}"
    );
    for (name, requirement) in &direct {
        assert_eq!(
            requirement,
            &format!("={recorded}"),
            "{name} must be pinned with ={recorded}"
        );
    }
    let locked = locked_gpui_packages();
    assert!(
        !locked.is_empty(),
        "Cargo.lock has gpui-unofficial packages"
    );
    for (name, version) in locked {
        assert_eq!(version, recorded, "Cargo.lock has {name} {version}");
        assert!(
            direct.iter().any(|(direct, _)| *direct == name),
            "{name} is in Cargo.lock but not a direct dependency of host"
        );
    }
}

/// The GPUI paths `clippy.toml` must disallow (contracts/bans-and-checks.md).
const BANNED: [&str; 17] = [
    "gpui::Window::prompt",
    "gpui::AsyncWindowContext::prompt",
    "gpui::PlatformWindow::prompt",
    "gpui::App::prompt_for_paths",
    "gpui::App::prompt_for_new_path",
    "gpui::App::open_url",
    "gpui::Platform::open_url",
    "gpui::App::open_with_system",
    "gpui::App::reveal_path",
    "gpui::App::write_credentials",
    "gpui::App::read_credentials",
    "gpui::App::delete_credentials",
    "gpui::img",
    "gpui::svg",
    "gpui::Svg::external_path",
    "gpui::Window::use_asset",
    "gpui::App::fetch_asset",
];

/// HS-12, FR-019, FR-029: every ban is in `clippy.toml`.
#[test]
fn clippy_bans_gpuis_prompts_openers_and_loaders() {
    let clippy = read("clippy.toml");
    for path in BANNED {
        assert!(
            clippy.contains(&format!("path = \"{path}\"")),
            "clippy.toml no longer disallows {path}"
        );
    }
}

/// R12: no way around the bans: only `media.rs` may allow a disallowed method or type.
#[test]
fn only_media_allows_disallowed_methods() {
    for (name, text) in host_sources() {
        if name == "host/src/media.rs" {
            continue;
        }
        for lint in ["disallowed_methods", "disallowed_types"] {
            assert!(
                !code(&text).contains(&format!("clippy::{lint}")),
                "{name} allows clippy::{lint}"
            );
        }
    }
}

/// R6: no floating layer, which could draw over the chrome.
#[test]
fn no_floating_layer() {
    for (name, text) in host_sources() {
        for call in ["deferred(", "anchored("] {
            assert!(!code(&text).contains(call), "{name} calls {call}");
        }
    }
}

/// FR-041: the window never turns the runtime's development mode on.
#[test]
fn no_development_mode() {
    for (name, text) in host_sources() {
        assert!(
            !code(&text).contains("development("),
            "{name} turns development mode on"
        );
    }
}

/// FR-039, R7: no host text written outside the catalog. A string literal handed straight to an
/// element, a label, a description, a tooltip or the window title is refused.
#[test]
fn host_words_come_from_the_catalog() {
    let patterns = [
        ".child(\"",
        ".children([\"",
        "aria_label(\"",
        "aria_description(\"",
        "tooltip(\"",
        "title: Some(\"",
        "SharedString::from(\"",
        "SharedString::new_static(\"",
        "Text::new(\"",
    ];
    for (name, text) in host_sources() {
        if name == "host/src/text.rs" {
            continue;
        }
        for pattern in patterns {
            assert!(
                !code(&text).contains(pattern),
                "{name} writes host text outside host/src/text.rs: {pattern}"
            );
        }
    }
}

/// R6 of feature 007, HS-12: `deny.toml` keeps `host-surfaces` to `host` and bans the native
/// dialog crates.
#[test]
fn deny_keeps_the_host_surfaces_and_dialog_bans() {
    let deny = read("deny.toml");
    assert!(deny.contains("{ name = \"host-surfaces\", wrappers = [\"host\"] }"));
    assert!(deny.contains("{ name = \"rfd\" }"));
    assert!(deny.contains("{ name = \"native-dialog\" }"));
}
