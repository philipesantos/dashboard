//! Every vendored file under design/ is the upstream file, unmodified (specs/007-host-visual-design,
//! T007, research R1, R2, R9): its SHA-256 must equal the one design/README.md records. Text
//! files are hashed after turning \r\n into \n, so a checkout on Windows gives the same hash.

use sha2::{Digest, Sha256};
use std::path::Path;

fn design_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The `| `file` | `hash` |` rows of design/README.md's tables.
fn recorded() -> Vec<(String, String)> {
    let readme = std::fs::read_to_string(design_dir().join("README.md")).expect("design/README.md");
    readme
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            // ["", "`file`", "`hash`", ""]
            if cells.len() != 4 {
                return None;
            }
            let file = cells[1].strip_prefix('`')?.strip_suffix('`')?;
            let hash = cells[2].strip_prefix('`')?.strip_suffix('`')?;
            (hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
                .then(|| (file.to_owned(), hash.to_owned()))
        })
        .collect()
}

fn digest(file: &str, bytes: Vec<u8>) -> String {
    let binary = file.ends_with(".ttf") || file.ends_with(".woff2");
    let bytes = if binary {
        bytes
    } else {
        String::from_utf8(bytes)
            .unwrap_or_else(|_| panic!("{file} is not UTF-8"))
            .replace("\r\n", "\n")
            .into_bytes()
    };
    Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn every_vendored_file_matches_its_recorded_hash() {
    let pins = recorded();
    // Radix (4), the format (2), Barlow (7), IBM Plex Mono (5)
    assert_eq!(
        pins.len(),
        18,
        "design/README.md should record eighteen files: {pins:?}"
    );
    for (file, pinned) in pins {
        let bytes = std::fs::read(design_dir().join(&file))
            .unwrap_or_else(|error| panic!("design/{file}: {error}"));
        assert_eq!(
            digest(&file, bytes),
            pinned,
            "design/{file} changed: a vendored file is upgraded in its own reviewed commit, with \
             its new hash in design/README.md"
        );
    }
}

#[test]
fn a_changed_byte_changes_the_hash() {
    let original = digest("x.ts", b"export const gray = {\n};\n".to_vec());
    let changed = digest("x.ts", b"export const grey = {\n};\n".to_vec());
    let crlf = digest("x.ts", b"export const gray = {\r\n};\r\n".to_vec());
    assert_ne!(original, changed);
    assert_eq!(
        original, crlf,
        "line endings must not change a text file's hash"
    );
}
