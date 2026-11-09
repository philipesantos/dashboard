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

/// The version a `host/README.md` records for GPUI.
fn recorded_gpui_version(readme: &str) -> Option<String> {
    let line = readme
        .lines()
        .find(|line| line.starts_with("| Version |"))?;
    line.split('`').nth(1).map(str::to_owned)
}

/// The gpui-unofficial packages a `host/Cargo.toml` depends on, with their requirement.
fn host_gpui_dependencies(manifest: &str) -> Vec<(String, String)> {
    manifest
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
                .unwrap_or("")
                .to_owned();
            (name, version)
        })
        .collect()
}

/// `(name, version)` of every package in a `Cargo.lock` whose name holds `gpui-unofficial`.
fn locked_gpui_packages(lock: &str) -> Vec<(String, String)> {
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

/// What breaks R1 and FR-003 in a README, a host manifest and a lock file: every gpui-unofficial
/// package a direct `=` dependency of `host` at the version the README records, never a
/// pre-release, all nineteen of them.
fn pin_problems(readme: &str, manifest: &str, lock: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(recorded) = recorded_gpui_version(readme) else {
        return vec!["host/README.md does not record the GPUI version".to_owned()];
    };
    if recorded.contains('-') {
        problems.push(format!("the recorded version {recorded} is a pre-release"));
    }
    // The dev-dependency with GPUI's test support names gpui-unofficial again, pinned the same
    let direct = host_gpui_dependencies(manifest);
    let mut names: Vec<&str> = direct.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    if names.len() != 19 {
        problems.push(format!("not nineteen gpui-unofficial crates: {names:?}"));
    }
    for (name, requirement) in &direct {
        if *requirement != format!("={recorded}") {
            problems.push(format!(
                "{name} is {requirement:?}, not pinned with ={recorded}"
            ));
        }
    }
    let locked = locked_gpui_packages(lock);
    if locked.is_empty() {
        problems.push("Cargo.lock has no gpui-unofficial package".to_owned());
    }
    for (name, version) in locked {
        if version != recorded {
            problems.push(format!("Cargo.lock has {name} {version}, not {recorded}"));
        }
        if !direct.iter().any(|(direct, _)| *direct == name) {
            problems.push(format!(
                "{name} is in Cargo.lock but not a direct dependency of host"
            ));
        }
    }
    problems
}

/// R1, FR-003: every gpui-unofficial package is a direct `=` dependency of `host`, at the version
/// `host/README.md` records, never a pre-release.
#[test]
fn gpui_is_pinned_as_recorded() {
    let problems = pin_problems(
        &read("host/README.md"),
        &read("host/Cargo.toml"),
        &read("Cargo.lock"),
    );
    assert!(problems.is_empty(), "{problems:#?}");
    // The fixture that must fail clippy builds the same GPUI
    let recorded = recorded_gpui_version(&read("host/README.md")).expect("recorded");
    assert_eq!(
        host_gpui_dependencies(&read("host/ban-fixture/Cargo.toml")),
        [("gpui-unofficial".to_owned(), format!("={recorded}"))],
        "host/ban-fixture pins GPUI as host does"
    );
}

/// US6, T044: the pin check refuses each way a pin can slip: a GPUI crate unpinned, with a caret,
/// at a pre-release or at a version the README does not record; a pre-release recorded; a GPUI
/// crate in the lock file that `host` does not name.
#[test]
fn the_pin_check_catches_every_slip() {
    let (readme, manifest, lock) = (
        read("host/README.md"),
        read("host/Cargo.toml"),
        read("Cargo.lock"),
    );
    let recorded = recorded_gpui_version(&readme).expect("recorded");
    let pin = format!("collections-gpui-unofficial = \"={recorded}\"");
    assert!(manifest.contains(&pin), "the manifest has {pin}");
    let with_pin = |requirement: String| {
        manifest.replace(
            &pin,
            &format!("collections-gpui-unofficial = \"{requirement}\""),
        )
    };
    let with_recorded = |version: &str| {
        readme.replace(
            &format!("| Version | `{recorded}` |"),
            &format!("| Version | `{version}` |"),
        )
    };
    let pre = format!("{recorded}-pre");
    let mutations = [
        (
            "unpinned",
            readme.clone(),
            with_pin(recorded.clone()),
            lock.clone(),
        ),
        (
            "caret",
            readme.clone(),
            with_pin(format!("^{recorded}")),
            lock.clone(),
        ),
        (
            "pre-release pin",
            readme.clone(),
            with_pin(format!("={pre}")),
            lock.clone(),
        ),
        (
            "differs from the README",
            with_recorded("1.99.0"),
            manifest.clone(),
            lock.clone(),
        ),
        (
            "pre-release recorded",
            with_recorded(&pre),
            manifest.replace(&format!("\"={recorded}\""), &format!("\"={pre}\"")),
            lock.replace(
                &format!("version = \"{recorded}\""),
                &format!("version = \"{pre}\""),
            ),
        ),
        (
            "not a direct dependency",
            readme.clone(),
            manifest.replace(&format!("{pin}\n"), ""),
            lock.clone(),
        ),
    ];
    for (slip, readme, manifest, lock) in mutations {
        assert!(
            !pin_problems(&readme, &manifest, &lock).is_empty(),
            "the pin check let through: {slip}"
        );
    }
}

/// The GPUI paths `clippy.toml` must disallow (contracts/bans-and-checks.md).
const BANNED: [&str; 41] = [
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
    "gpui::Window::get_asset",
    "gpui::ImageCacheItem::new",
    "gpui::RetainAllImageCache::load",
    "gpui::AnyImageCache::load",
    "gpui::ImageCache::load",
    "gpui::Platform::prompt_for_paths",
    "gpui::Platform::prompt_for_new_path",
    "gpui::Platform::open_with_system",
    "gpui::Platform::reveal_path",
    "gpui::Platform::write_credentials",
    "gpui::Platform::read_credentials",
    "gpui::Platform::delete_credentials",
    "gpui::Window::paint_svg",
    "gpui::Asset::load",
    "gpui::Image::to_image_data",
    "gpui::Image::use_render_image",
    "gpui::Image::get_render_image",
    "gpui::App::svg_renderer",
    "gpui::SvgRenderer::new",
    "gpui::SvgRenderer::parse_svg",
    "gpui::SvgRenderer::render_parsed",
    "gpui::SvgRenderer::render_single_frame",
    "gpui::App::reset_prompt_builder",
    "gpui::fallback_prompt_renderer",
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

/// The GPUI source the lock file pins, by package: every gpui-unofficial crate whose source Cargo
/// has downloaded (the crates for other systems are absent until a build for them needs them).
fn gpui_sources() -> Vec<(String, PathBuf)> {
    let version = recorded_gpui_version(&read("host/README.md")).expect("recorded");
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".cargo"))
        })
        .expect("a Cargo home");
    let registries: Vec<PathBuf> = fs::read_dir(cargo_home.join("registry/src"))
        .map(|entries| {
            entries
                .filter_map(|entry| Some(entry.ok()?.path()))
                .collect()
        })
        .unwrap_or_default();
    let mut names: Vec<String> = locked_gpui_packages(&read("Cargo.lock"))
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    names.sort();
    names.dedup();
    let found: Vec<(String, PathBuf)> = names
        .into_iter()
        .filter_map(|name| {
            let dir = registries
                .iter()
                .map(|registry| registry.join(format!("{name}-{version}")))
                .find(|dir| dir.join("src").is_dir())?;
            Some((name, dir))
        })
        .collect();
    assert!(
        found.iter().any(|(name, _)| name == "gpui-unofficial"),
        "gpui-unofficial {version} not found under {}: build the host first",
        cargo_home.display()
    );
    found
}

/// The name a crate is used by: its `[lib]` name, or its package name with `_` for `-`.
fn lib_name(name: &str, dir: &Path) -> String {
    let manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
    let mut in_lib = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_lib = line == "[lib]";
        } else if in_lib
            && let Some(value) = line.strip_prefix("name")
            && let Some(value) = value.trim_start().strip_prefix('=')
        {
            return value.trim().trim_matches('"').to_owned();
        }
    }
    name.replace('-', "_")
}

/// Rust source with comments removed and string and character literals emptied (line breaks
/// kept), so the braces left are the code's.
fn strip_literals(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let at = |i: usize, text: &str| {
        text.chars()
            .enumerate()
            .all(|(k, c)| chars.get(i + k) == Some(&c))
    };
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    while i < chars.len() {
        let c = chars[i];
        if at(i, "//") {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if at(i, "/*") {
            let mut depth = 0;
            while i < chars.len() {
                if at(i, "/*") {
                    depth += 1;
                    i += 2;
                } else if at(i, "*/") {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if chars[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            continue;
        }
        let before_is_ident = i > 0 && ident(chars[i - 1]);
        // Raw strings: r"…", r#"…"#, br#"…"#
        let raw_start = if !before_is_ident && at(i, "br") {
            Some(i + 2)
        } else if !before_is_ident && c == 'r' {
            Some(i + 1)
        } else {
            None
        };
        if let Some(mut j) = raw_start {
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) == Some(&'"') {
                let end: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                j += 1;
                let mut lines = 0;
                while j < chars.len() && !at(j, &end) {
                    if chars[j] == '\n' {
                        lines += 1;
                    }
                    j += 1;
                }
                out.push_str("\"\"");
                out.extend(std::iter::repeat_n('\n', lines));
                i = j + end.chars().count();
                continue;
            }
        }
        if c == '"' || (c == 'b' && chars.get(i + 1) == Some(&'"') && !before_is_ident) {
            i += if c == '"' { 1 } else { 2 };
            let mut lines = 0;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                if chars.get(i) == Some(&'\n') {
                    lines += 1;
                }
                i += 1;
            }
            i += 1;
            out.push_str("\"\"");
            out.extend(std::iter::repeat_n('\n', lines));
            continue;
        }
        if c == '\'' {
            // A character literal, not a lifetime: 'x', '\n', '\u{..}', '\x..'
            let close = if chars.get(i + 1) == Some(&'\\') {
                (i + 2..(i + 12).min(chars.len())).find(|&k| chars[k] == '\'')
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            if let Some(close) = close {
                out.push_str("' '");
                i = close + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// `text` without its attributes (`#[…]`, `#![…]`).
fn without_attributes(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '#' && matches!(chars.peek(), Some('[') | Some('!')) {
            let mut depth = 0;
            for c in chars.by_ref() {
                match c {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            continue;
        }
        out.push(c);
    }
    out.trim().to_owned()
}

/// The identifier after the word `word` in `text`, if `word` appears as a whole word.
fn word_then_ident(text: &str, word: &str) -> Option<String> {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(found) = text[from..].find(word) {
        let start = from + found;
        let end = start + word.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        if before.is_none_or(|c| !ident(c)) && after.is_some_and(char::is_whitespace) {
            let name: String = text[end..]
                .trim_start()
                .chars()
                .take_while(|c| ident(*c))
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
        from = end;
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
enum Scope {
    /// An inherent `impl` of the named type: its `pub fn`s are public.
    Impl(String),
    /// A `pub trait`: every `fn` in it is public.
    Trait(String),
    /// A trait impl or a private trait: its functions are not public ones.
    Hidden,
    /// A function's body: nothing in it is public.
    Body,
}

/// Every public function of a crate's source, as `crate::Type::function` or `crate::function`
/// (modules left out, as GPUI re-exports them at its root), with its signature.
fn public_functions(lib: &str, dir: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&dir.join("src"), &mut files);
    let mut found = Vec::new();
    for file in files {
        let source = strip_literals(&fs::read_to_string(&file).unwrap_or_default());
        // (depth it opened at, scope); `None` for blocks that change nothing (mod, struct, …)
        let mut scopes: Vec<(usize, Option<Scope>)> = Vec::new();
        let mut depth = 0;
        let mut statement = String::new();
        for c in source.chars() {
            if !matches!(c, '{' | '}' | ';') {
                statement.push(c);
                continue;
            }
            let head = without_attributes(&statement);
            statement.clear();
            let inner = scopes.iter().rev().find_map(|(_, scope)| scope.clone());
            let public = |head: &str| {
                let before_fn = head.split("fn ").next().unwrap_or("");
                let mut words = before_fn.split_whitespace();
                words.next() == Some("pub")
                    && words.all(|word| {
                        matches!(word, "const" | "async" | "unsafe" | "extern" | "\"\"")
                    })
            };
            let function = word_then_ident(&head, "fn").filter(|_| {
                !head.starts_with("impl")
                    && !head.starts_with("unsafe impl")
                    && word_then_ident(&head, "trait").is_none()
            });
            if let Some(name) = &function
                && c != '}'
            {
                let path = match &inner {
                    _ if scopes.iter().any(|(_, scope)| scope == &Some(Scope::Body)) => None,
                    None if public(&head) => Some(format!("{lib}::{name}")),
                    Some(Scope::Impl(ty)) if public(&head) => Some(format!("{lib}::{ty}::{name}")),
                    Some(Scope::Trait(tr)) => Some(format!("{lib}::{tr}::{name}")),
                    _ => None,
                };
                if let Some(path) = path {
                    found.push((path, head.clone()));
                }
            }
            match c {
                '{' => {
                    depth += 1;
                    let scope = if function.is_some() {
                        Some(Scope::Body)
                    } else if let Some(rest) = head
                        .strip_prefix("unsafe impl")
                        .or_else(|| head.strip_prefix("impl"))
                        .filter(|rest| rest.starts_with(|c: char| c.is_whitespace() || c == '<'))
                    {
                        if rest.split_whitespace().any(|word| word == "for") {
                            Some(Scope::Hidden)
                        } else {
                            // `impl<T> Name<T>` / `impl path::Name`: the type's last segment
                            let mut rest = rest.trim_start();
                            if rest.starts_with('<') {
                                let mut level = 0;
                                let end = rest
                                    .char_indices()
                                    .find(|&(_, c)| {
                                        level += match c {
                                            '<' => 1,
                                            '>' => -1,
                                            _ => 0,
                                        };
                                        level == 0
                                    })
                                    .map_or(rest.len(), |(at, _)| at + 1);
                                rest = rest[end..].trim_start();
                            }
                            let path = rest
                                .split(|c: char| c == '<' || c.is_whitespace())
                                .next()
                                .unwrap_or("");
                            Some(Scope::Impl(
                                path.rsplit("::").next().unwrap_or(path).to_owned(),
                            ))
                        }
                    } else if let Some(name) = word_then_ident(&head, "trait") {
                        let words: Vec<&str> = head.split_whitespace().collect();
                        let at = words.iter().position(|word| *word == "trait").unwrap_or(0);
                        let public = words[..at]
                            .iter()
                            .filter(|word| **word != "unsafe")
                            .eq(["pub"].iter());
                        Some(if public {
                            Scope::Trait(name)
                        } else {
                            Scope::Hidden
                        })
                    } else {
                        None
                    };
                    scopes.push((depth, scope));
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    while scopes.last().is_some_and(|(opened, _)| *opened > depth) {
                        scopes.pop();
                    }
                }
                _ => {}
            }
        }
    }
    found
}

/// Whether a public function's name, an argument's name or an argument's type says it may load,
/// read, open, prompt, decode, reach the network or the keychain. A word filter: a function named
/// and typed like none of these still passes, so GPUI upgrades are also read by hand.
fn may_reach_out(path: &str, signature: &str) -> bool {
    const NAMES: [&str; 17] = [
        "open",
        "prompt",
        "credential",
        "reveal",
        "asset",
        "image",
        "img",
        "svg",
        "url",
        "uri",
        "fetch",
        "load",
        "file",
        "http",
        "download",
        "keychain",
        "clipboard",
    ];
    const TYPES: [&str; 11] = [
        "Resource",
        "Path",
        "PathBuf",
        "Uri",
        "Url",
        "ImageSource",
        "SharedUri",
        "HttpClient",
        "AssetSource",
        "Image",
        "RenderImage",
    ];
    const ARGUMENTS: [&str; 16] = [
        "path", "paths", "url", "urls", "uri", "src", "source", "sources", "file", "files",
        "filename", "location", "address", "href", "link", "resource",
    ];
    let name = path.rsplit("::").next().unwrap_or(path);
    // `fn x(path: impl Into<SharedString>)`: an argument named for what it reaches
    let argument_named = signature.split(['(', ',']).skip(1).any(|piece| {
        let piece = piece.trim_start();
        let piece = piece.strip_prefix("mut ").unwrap_or(piece);
        piece
            .split_once(':')
            .is_some_and(|(argument, _)| ARGUMENTS.contains(&argument.trim()))
    });
    argument_named
        || NAMES.iter().any(|word| name.contains(word))
        || signature
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|word| TYPES.contains(&word))
}

/// `host/gpui-reviewed.txt`: each GPUI function allowed, with the reason heading its group.
fn reviewed_gpui_functions() -> Vec<(String, String)> {
    let mut reason = None;
    let mut reviewed = Vec::new();
    for line in read("host/gpui-reviewed.txt").lines() {
        let line = line.trim();
        if let Some(heading) = line.strip_prefix("## ") {
            reason = Some(heading.to_owned());
        } else if !line.is_empty() && !line.starts_with('#') {
            let reason = reason
                .clone()
                .unwrap_or_else(|| panic!("{line} has no reason above it"));
            reviewed.push((line.to_owned(), reason));
        }
    }
    reviewed
}

/// T056: every public GPUI function whose name or signature may load, read, open, prompt,
/// decode, or reach the network or the keychain is banned in `clippy.toml` or reviewed in
/// `host/gpui-reviewed.txt`, so a GPUI upgrade that adds one fails here until someone decides;
/// a reviewed one that is gone, and a ban that names nothing, fail too.
#[test]
fn every_gpui_way_out_is_banned_or_reviewed() {
    let reviewed = reviewed_gpui_functions();
    let mut problems = Vec::new();
    let mut scanned_libs = Vec::new();
    let mut all = std::collections::BTreeSet::new();
    let mut flagged = std::collections::BTreeSet::new();
    for (name, dir) in gpui_sources() {
        let lib = lib_name(&name, &dir);
        for (path, signature) in public_functions(&lib, &dir) {
            if may_reach_out(&path, &signature) {
                flagged.insert(path.clone());
            }
            all.insert(path);
        }
        scanned_libs.push(lib);
    }
    for path in &flagged {
        let banned = BANNED.contains(&path.as_str());
        let allowed = reviewed.iter().any(|(entry, _)| entry == path);
        if !banned && !allowed {
            problems.push(format!(
                "{path}: new in GPUI and neither banned in clippy.toml nor reviewed in host/gpui-reviewed.txt"
            ));
        }
        if banned && allowed {
            problems.push(format!("{path}: both banned and reviewed"));
        }
    }
    for (entry, _) in &reviewed {
        let lib = entry.split("::").next().unwrap_or("");
        if scanned_libs.iter().any(|scanned| scanned == lib) && !flagged.contains(entry) {
            problems.push(format!("{entry}: reviewed, but GPUI no longer has it"));
        }
    }
    for path in BANNED {
        if !all.contains(path) {
            problems.push(format!(
                "{path}: banned, but GPUI has no such public function"
            ));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// T056: `host/src` names no GPUI crate for one system (or for WebAssembly), whose functions the
/// review above covers only on that system.
#[test]
fn host_names_no_os_specific_gpui_crate() {
    for (name, text) in host_sources() {
        for crate_name in [
            "gpui_linux",
            "gpui_macos",
            "gpui_apple",
            "gpui_windows",
            "gpui_web",
            "gpui_wgpu",
        ] {
            assert!(
                !code(&text).contains(crate_name),
                "{name} names {crate_name}"
            );
        }
    }
}

/// T056: the scanner reads what it must: comments and literals do not move its braces, trait
/// impls and private items are not public, and `pub trait` methods are.
#[test]
fn the_gpui_scanner_reads_rust() {
    let dir = tempfile::TempDir::new().expect("a directory");
    fs::create_dir(dir.path().join("src")).expect("src");
    fs::write(
        dir.path().join("src/lib.rs"),
        r##"
        // pub fn in_a_comment() {
        /* pub fn in_a_block() { */
        pub fn img(source: &str) -> Img { let s = "}{"; let c = '{'; let r = r#"}"#; todo!() }
        fn private_free() {}
        pub(crate) fn crate_only() {}
        pub struct Window { field: u8 }
        impl Window {
            pub fn open_url(&self) { if true { let _ = |x: u8| { x }; } }
            fn private_method(&self) {}
            pub async fn load_later(&self) {}
        }
        impl<T> Clone for Holder<T> { fn clone(&self) -> Self { todo!() } }
        impl<'a, T: Into<u8>> crate::Holder<T> { pub fn fetch(&self, path: &Path) {} }
        pub trait Platform { fn reveal_path(&self, path: &Path); fn other(&self) {} }
        trait Private { fn hidden(&self); }
        mod nested { pub fn nested_free(resource: Resource) {} }
        "##,
    )
    .expect("written");
    let mut found: Vec<String> = public_functions("demo", dir.path())
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            "demo::Holder::fetch",
            "demo::Platform::other",
            "demo::Platform::reveal_path",
            "demo::Window::load_later",
            "demo::Window::open_url",
            "demo::img",
            "demo::nested_free",
        ]
    );
    assert!(may_reach_out(
        "demo::nested_free",
        "pub fn nested_free(resource: Resource)"
    ));
    assert!(!may_reach_out("demo::Platform::other", "fn other(&self)"));
    // Named and typed like nothing above, but its argument is named for what it reads
    assert!(may_reach_out(
        "demo::Svg::external_path",
        "pub fn external_path(mut self, path: impl Into<SharedString>) -> Self"
    ));
}
