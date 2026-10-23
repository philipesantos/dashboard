//! The plugin package: one WebAssembly component whose manifest rides in one top-level custom
//! section (`specs/002-plugin-permissions/contracts/package.md`, research R1).

use std::fmt::Write;

use sha2::{Digest, Sha256};
use wasmparser::{Encoding, Parser, Payload};

use crate::error::{ErrorKind, RuntimeError};
use crate::manifest::Manifest;

/// The custom section that carries a plugin's manifest.
pub const MANIFEST_SECTION: &str = "dashboard-manifest";

/// The id of a custom section in the binary format.
const CUSTOM_SECTION_ID: u8 = 0;

/// The payload of the package's only manifest section, found without compiling the package.
///
/// Refuses a package with no manifest section as `no-manifest`; one with two or more at the top
/// level, or one inside a nested module or component, as `manifest-mismatch`; one that is not a
/// well-formed component as `invalid-package`.
pub fn manifest_section(package: &[u8]) -> Result<&[u8], RuntimeError> {
    // Walks the section headers of the package and of every nested module and component,
    // without validating or compiling anything.
    let mut depth = 0_usize;
    let mut top_level = Vec::new();
    let mut nested = false;
    for payload in Parser::new(0).parse_all(package) {
        let payload = payload
            .map_err(|error| RuntimeError::new(ErrorKind::InvalidPackage, error.to_string()))?;
        match payload {
            Payload::Version { encoding, .. } => {
                if depth == 0 && encoding != Encoding::Component {
                    return Err(RuntimeError::new(
                        ErrorKind::InvalidPackage,
                        "not a WebAssembly component",
                    ));
                }
                depth += 1;
            }
            Payload::End(_) => depth -= 1,
            Payload::CustomSection(section) if section.name() == MANIFEST_SECTION => {
                if depth == 1 {
                    top_level.push(section.data());
                } else {
                    nested = true;
                }
            }
            _ => {}
        }
    }

    if nested {
        return Err(RuntimeError::new(
            ErrorKind::ManifestMismatch,
            format!(
                "a {MANIFEST_SECTION} section sits inside a nested module or component; the manifest must be one top-level section"
            ),
        ));
    }
    match top_level[..] {
        [manifest] => Ok(manifest),
        [] => Err(RuntimeError::new(
            ErrorKind::NoManifest,
            format!("the package has no {MANIFEST_SECTION} section"),
        )),
        _ => Err(RuntimeError::new(
            ErrorKind::ManifestMismatch,
            format!(
                "the package has {} {MANIFEST_SECTION} sections; it must have exactly one",
                top_level.len()
            ),
        )),
    }
}

/// The package digest: `sha256:` and the SHA-256 of the whole file, in lowercase hex.
pub fn digest(package: &[u8]) -> String {
    let mut digest = String::from("sha256:");
    for byte in Sha256::digest(package) {
        write!(digest, "{byte:02x}").expect("writing to a String never fails");
    }
    digest
}

/// Checks `manifest` and appends it to `component` as its manifest section, at the end.
///
/// Refuses an invalid manifest as `invalid-manifest` or `unknown-permission`, and a component
/// that already has a manifest section as `manifest-mismatch`.
pub fn pack(component: &[u8], manifest: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    Manifest::parse(manifest)?;
    match manifest_section(component) {
        Err(error) if error.kind() == ErrorKind::NoManifest => {}
        Ok(_) => {
            return Err(RuntimeError::new(
                ErrorKind::ManifestMismatch,
                format!("the component already has a {MANIFEST_SECTION} section"),
            ));
        }
        Err(error) => return Err(error),
    }

    // A custom section: its id, the size of what follows, the name's length and bytes, then
    // the payload as it is.
    let mut contents = Vec::with_capacity(MANIFEST_SECTION.len() + manifest.len() + 1);
    leb128(MANIFEST_SECTION.len(), &mut contents);
    contents.extend_from_slice(MANIFEST_SECTION.as_bytes());
    contents.extend_from_slice(manifest);
    let mut package = Vec::with_capacity(component.len() + contents.len() + 6);
    package.extend_from_slice(component);
    package.push(CUSTOM_SECTION_ID);
    leb128(contents.len(), &mut package);
    package.extend_from_slice(&contents);
    Ok(package)
}

/// Appends `value` in unsigned LEB128, the binary format's encoding of sizes.
fn leb128(mut value: usize, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

#[cfg(test)]
mod tests {
    use wasmtime::Engine;
    use wasmtime::component::Component;

    use super::*;

    const MANIFEST: &str = r#"{"manifest_version":1,"id":"local/probe","version":"1.0.0","author":"A","permissions":[],"login_providers":[]}"#;

    /// [`MANIFEST`] requesting `permission`, required.
    fn manifest_requesting(permission: &str) -> String {
        MANIFEST.replace(
            r#""permissions":[]"#,
            &format!(r#""permissions":[{{"name":"{permission}","required":true}}]"#),
        )
    }

    /// `text` as a WAT string literal.
    fn wat_string(text: &str) -> String {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }

    /// A component built from WAT, with `fields` inside it.
    fn component(fields: &str) -> Vec<u8> {
        wat::parse_str(format!("(component {fields})")).expect("the test component is valid WAT")
    }

    fn manifest_field(payload: &str) -> String {
        format!(
            "(@custom {} {})",
            wat_string(MANIFEST_SECTION),
            wat_string(payload)
        )
    }

    fn assert_refused(package: &[u8], kind: ErrorKind) -> String {
        let error = manifest_section(package).expect_err("the package is refused");
        assert_eq!(error.kind(), kind, "{error}");
        error.detail().to_owned()
    }

    #[test]
    fn finds_the_one_top_level_section() {
        let package = component(&manifest_field(MANIFEST));
        assert_eq!(manifest_section(&package), Ok(MANIFEST.as_bytes()));

        // Anywhere among the top-level sections
        let package = component(&format!(
            "{} (core module) (@custom \"other\" \"x\")",
            manifest_field(MANIFEST)
        ));
        assert_eq!(manifest_section(&package), Ok(MANIFEST.as_bytes()));
    }

    #[test]
    fn a_package_without_the_section_has_no_manifest() {
        assert_refused(&component(""), ErrorKind::NoManifest);
        // Another name is not the manifest, however close
        let package = component(&format!(
            "(@custom \"dashboard-manifest.json\" {}) (@custom \"Dashboard-Manifest\" {})",
            wat_string(MANIFEST),
            wat_string(MANIFEST)
        ));
        assert_refused(&package, ErrorKind::NoManifest);
    }

    #[test]
    fn two_top_level_sections_are_a_mismatch() {
        let package = component(&format!(
            "{} {}",
            manifest_field(MANIFEST),
            manifest_field(&manifest_requesting("log"))
        ));
        assert_refused(&package, ErrorKind::ManifestMismatch);

        // The same manifest twice too: there must be exactly one
        let package = component(&format!(
            "{} {}",
            manifest_field(MANIFEST),
            manifest_field(MANIFEST)
        ));
        assert_refused(&package, ErrorKind::ManifestMismatch);
    }

    #[test]
    fn a_section_in_a_nested_module_or_component_is_a_mismatch() {
        let nested_module = component(&format!("(core module {})", manifest_field(MANIFEST)));
        assert_refused(&nested_module, ErrorKind::ManifestMismatch);

        let nested_component = component(&format!("(component {})", manifest_field(MANIFEST)));
        assert_refused(&nested_component, ErrorKind::ManifestMismatch);

        // Even beside a top-level one: a tool could read either
        let both = component(&format!(
            "{} (core module {})",
            manifest_field(MANIFEST),
            manifest_field(MANIFEST)
        ));
        assert_refused(&both, ErrorKind::ManifestMismatch);
    }

    #[test]
    fn a_package_that_is_not_a_component_is_invalid() {
        let module = wat::parse_str(format!("(module {})", manifest_field(MANIFEST)))
            .expect("the test module is valid WAT");
        assert_refused(&module, ErrorKind::InvalidPackage);
        assert_refused(b"not wasm", ErrorKind::InvalidPackage);

        let mut truncated = component(&manifest_field(MANIFEST));
        truncated.truncate(truncated.len() - 1);
        assert_refused(&truncated, ErrorKind::InvalidPackage);
    }

    #[test]
    fn the_digest_is_the_sha256_of_the_whole_file() {
        // SHA-256 of the empty string and of "abc" (FIPS 180-2 test vectors)
        assert_eq!(
            digest(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            digest(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let package = component(&manifest_field(MANIFEST));
        let other = component(&manifest_field(&MANIFEST.replace("1.0.0", "1.0.1")));
        assert_ne!(digest(&package), digest(&other));
    }

    #[test]
    fn pack_appends_a_section_the_finder_returns() {
        let bare = component("(core module)");
        let packed = pack(&bare, MANIFEST.as_bytes()).expect("packed");
        assert!(packed.starts_with(&bare), "the component is kept as it was");
        assert_eq!(manifest_section(&packed), Ok(MANIFEST.as_bytes()));
        Component::from_binary(&Engine::default(), &packed).expect("the package still compiles");

        // A manifest long enough to need a two-byte size
        let long = MANIFEST.replace("\"A\"", &format!("\"{}\"", "a".repeat(100)));
        let packed = pack(&bare, long.as_bytes()).expect("packed");
        assert_eq!(manifest_section(&packed), Ok(long.as_bytes()));
    }

    #[test]
    fn pack_refuses_a_component_that_has_a_manifest() {
        let packed = pack(&component(""), MANIFEST.as_bytes()).expect("packed");
        let error = pack(&packed, MANIFEST.as_bytes()).expect_err("packed twice");
        assert_eq!(error.kind(), ErrorKind::ManifestMismatch);

        let nested = component(&format!("(core module {})", manifest_field(MANIFEST)));
        let error = pack(&nested, MANIFEST.as_bytes()).expect_err("a nested manifest");
        assert_eq!(error.kind(), ErrorKind::ManifestMismatch);
    }

    #[test]
    fn pack_refuses_an_invalid_manifest_or_package() {
        let bare = component("");
        let error = pack(&bare, b"{}").expect_err("an empty object");
        assert_eq!(error.kind(), ErrorKind::InvalidManifest);

        let telepathy = manifest_requesting("telepathy");
        let error = pack(&bare, telepathy.as_bytes()).expect_err("an unknown permission");
        assert_eq!(error.kind(), ErrorKind::UnknownPermission);

        let error = pack(b"not wasm", MANIFEST.as_bytes()).expect_err("not a component");
        assert_eq!(error.kind(), ErrorKind::InvalidPackage);
    }
}
