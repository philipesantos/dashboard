//! The plugin package: one WebAssembly component whose manifest rides in one top-level custom
//! section (`specs/002-plugin-permissions/contracts/package.md`, research R1).

use crate::error::{ErrorKind, RuntimeError};

/// The custom section that carries a plugin's manifest.
pub const MANIFEST_SECTION: &str = "dashboard-manifest";

/// The payload of the package's only manifest section, found without compiling the package.
///
/// Refuses a package with no manifest section as `no-manifest`; one with two or more at the top
/// level, or one inside a nested module or component, as `manifest-mismatch`; one that is not a
/// well-formed component as `invalid-package`.
pub fn manifest_section(_package: &[u8]) -> Result<&[u8], RuntimeError> {
    Err(RuntimeError::new(
        ErrorKind::NoManifest,
        "not implemented yet",
    ))
}

/// The package digest: `sha256:` and the SHA-256 of the whole file, in lowercase hex.
pub fn digest(_package: &[u8]) -> String {
    String::new()
}

/// Checks `manifest` and appends it to `component` as its manifest section, at the end.
///
/// Refuses an invalid manifest as `invalid-manifest` or `unknown-permission`, and a component
/// that already has a manifest section as `manifest-mismatch`.
pub fn pack(_component: &[u8], _manifest: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    Err(RuntimeError::new(
        ErrorKind::InvalidManifest,
        "not implemented yet",
    ))
}

#[cfg(test)]
mod tests {
    use wasmtime::Engine;
    use wasmtime::component::Component;

    use super::*;

    const MANIFEST: &str = r#"{"manifest_version":1,"id":"local/probe","version":"1.0.0","author":"A","permissions":[],"login_providers":[]}"#;

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
            manifest_field(&MANIFEST.replace("[]}", "[{\"name\":\"log\",\"required\":true}]}"))
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

        let telepathy = MANIFEST.replace("[]}", "[{\"name\":\"telepathy\",\"required\":true}]}");
        let error = pack(&bare, telepathy.as_bytes()).expect_err("an unknown permission");
        assert_eq!(error.kind(), ErrorKind::UnknownPermission);

        let error = pack(b"not wasm", MANIFEST.as_bytes()).expect_err("not a component");
        assert_eq!(error.kind(), ErrorKind::InvalidPackage);
    }
}
