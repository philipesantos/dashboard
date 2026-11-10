//! Every `<style>` the UI inserts at run time must carry the response's nonce, or the content
//! policy refuses it and the component draws wrong with nothing in CI to say so
//! (specs/008-tauri-shell/research.md, R3). The probe opens only the demo's dialog and select, so
//! this test checks the rest statically:
//! - the Radix packages whose parts draw a `<style>` and take a `nonce` prop are exactly the ones
//!   listed below, so a new package or version that starts taking one fails here;
//! - every use of such a part in `app/src` passes `nonce=`, which also catches `shadcn add`
//!   overwriting a component that had it;
//! - `index.html` keeps the empty `<style>` that receives the nonce, and `main.tsx` hands it to
//!   `get-nonce`, which Radix's scroll lock reads.
//!
//! Part of the security core (constitution, Principle IX).

use std::fs;
use std::path::{Path, PathBuf};

/// Radix packages with a part that takes `nonce`, and the JSX name of that part.
const NONCE_PARTS: &[(&str, &str)] = &[
    ("react-select", "SelectPrimitive.Viewport"),
    ("react-scroll-area", "ScrollAreaPrimitive.Viewport"),
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every installed `@radix-ui/react-*` package whose build mentions `nonce`.
fn radix_packages_with_nonce() -> Vec<String> {
    let store = repo().join("node_modules/.pnpm");
    let entries = fs::read_dir(&store)
        .unwrap_or_else(|error| panic!("{}: {error}; run pnpm install first", store.display()));
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("@radix-ui+react-") {
            continue;
        }
        let Ok(scoped) = fs::read_dir(entry.path().join("node_modules/@radix-ui")) else {
            continue;
        };
        for package in scoped.flatten() {
            let built = package.path().join("dist/index.mjs");
            if fs::read_to_string(&built).is_ok_and(|code| code.contains("nonce")) {
                found.push(package.file_name().to_string_lossy().into_owned());
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

fn tsx_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("app/src").flatten() {
        let path = entry.path();
        if path.is_dir() {
            tsx_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "tsx") {
            out.push(path);
        }
    }
}

#[test]
fn every_run_time_style_carries_the_nonce() {
    let mut failures = Vec::new();

    let mut listed: Vec<String> = NONCE_PARTS.iter().map(|(package, _)| package.to_string()).collect();
    listed.sort();
    for package in radix_packages_with_nonce() {
        if !listed.contains(&package) {
            failures.push(format!(
                "@radix-ui/{package} takes a nonce: add its part to NONCE_PARTS and pass nonce={{getNonce()}} where it is used"
            ));
        }
    }

    let mut files = Vec::new();
    tsx_files(&repo().join("app/src"), &mut files);
    for file in files {
        let source = fs::read_to_string(&file).expect("tsx source");
        for (_, part) in NONCE_PARTS {
            let opening = format!("<{part}");
            for (start, _) in source.match_indices(&opening) {
                let tag_end = source[start..].find('>').map_or(source.len(), |end| start + end);
                if !source[start..tag_end].contains("nonce=") {
                    let line = source[..start].lines().count() + 1;
                    failures.push(format!(
                        "{}:{line}: <{part}> draws a <style> and has no nonce= (pass nonce={{getNonce()}})",
                        file.strip_prefix(repo()).unwrap_or(&file).display()
                    ));
                }
            }
        }
    }

    let html = fs::read_to_string(repo().join("app/index.html")).expect("index.html");
    if !html.contains("<style></style>") {
        failures.push(
            "app/index.html: the empty <style> that receives Tauri's nonce is gone".to_string(),
        );
    }
    let main = fs::read_to_string(repo().join("app/src/main.tsx")).expect("main.tsx");
    if !main.contains("setNonce(") {
        failures.push("app/src/main.tsx: the nonce is no longer handed to get-nonce".to_string());
    }

    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}
