//! The published manifest schema, `wit/manifest.schema.json`, and the host's parser agree on
//! every manifest (research R2), and every manifest in this repository is valid (SC-006).

use std::path::{Path, PathBuf};

use boon::{Compiler, SchemaIndex, Schemas};
use runtime::ErrorKind;
use runtime::manifest::Manifest;
use serde_json::{Value, json};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the runtime crate has a parent directory")
        .to_path_buf()
}

/// The schema, compiled as an author's tools would compile it (JSON Schema draft 2020-12).
fn schema() -> (Schemas, SchemaIndex) {
    let path = repository().join("wit").join("manifest.schema.json");
    let mut schemas = Schemas::new();
    let index = Compiler::new()
        .compile(path.to_str().expect("a UTF-8 path"), &mut schemas)
        .unwrap_or_else(|error| panic!("the schema compiles: {error}"));
    (schemas, index)
}

fn valid_for_schema(schema: &(Schemas, SchemaIndex), manifest: &Value) -> bool {
    schema.0.validate(manifest, schema.1).is_ok()
}

/// The parser's verdict: `Ok` for a valid manifest, or the error kind.
fn parse(manifest: &Value) -> Result<(), ErrorKind> {
    let bytes = serde_json::to_vec(manifest).expect("a JSON value always serializes");
    Manifest::parse(&bytes)
        .map(drop)
        .map_err(|error| error.kind())
}

fn base() -> Value {
    json!({
        "manifest_version": 1,
        "id": "local/probe",
        "version": "1.0.0",
        "author": "Ada Lovelace",
        "permissions": [{ "name": "log", "required": true }],
        "login_providers": []
    })
}

/// The base manifest with `field` set to `value`, or removed when `value` is `None`.
fn with(field: &str, value: Option<Value>) -> Value {
    let mut manifest = base();
    let object = manifest.as_object_mut().expect("an object");
    match value {
        Some(value) => object.insert(field.to_owned(), value),
        None => object.remove(field),
    };
    manifest
}

fn set(field: &str, value: Value) -> Value {
    with(field, Some(value))
}

fn permission(name: Value, required: Value) -> Value {
    json!([{ "name": name, "required": required }])
}

/// Manifests both must accept, covering every rule of the format.
fn valid() -> Vec<(&'static str, Value)> {
    vec![
        ("the base manifest", base()),
        (
            "$schema",
            set("$schema", json!("../../../wit/manifest.schema.json")),
        ),
        (
            "$schema of 2048 characters",
            set("$schema", json!("x".repeat(2048))),
        ),
        ("manifest_version 1.0", set("manifest_version", json!(1.0))),
        ("a local id", set("id", json!("local/a"))),
        ("a local id with hyphens", set("id", json!("local/a--b-c"))),
        (
            "a local name of 64",
            set("id", json!(format!("local/{}", "a".repeat(64)))),
        ),
        (
            "a repository id",
            set("id", json!("github.com/owner-name/repo.name_x-y")),
        ),
        (
            "an owner of 39",
            set("id", json!(format!("github.com/{}/b", "a".repeat(39)))),
        ),
        (
            "a repository of 100",
            set("id", json!(format!("github.com/a/{}", "b".repeat(100)))),
        ),
        (
            "a repository starting with _",
            set("id", json!("github.com/a/_x")),
        ),
        (
            "a pre-release and build",
            set("version", json!("1.2.3-alpha.1+build.5")),
        ),
        (
            "a numeric pre-release",
            set("version", json!("1.0.0-0.3.7")),
        ),
        (
            "build digits with zeros",
            set("version", json!("1.0.0+001")),
        ),
        (
            "a version of 64",
            set("version", json!(format!("1.0.0-{}", "a".repeat(58)))),
        ),
        ("an author of 1", set("author", json!("A"))),
        ("an author of 100", set("author", json!("é".repeat(100)))),
        (
            "a bidirectional control",
            set("author", json!("\u{202e}dlrow")),
        ),
        ("no permissions", set("permissions", json!([]))),
        (
            "an optional permission",
            set("permissions", permission(json!("log"), json!(false))),
        ),
        (
            "32 permissions",
            set(
                "permissions",
                json!(vec![json!({ "name": "log", "required": true }); 32]),
            ),
        ),
        (
            "a login provider",
            set("login_providers", json!(["github"])),
        ),
    ]
}

/// Manifests both must refuse, covering every rule of the format.
fn invalid() -> Vec<(&'static str, Value)> {
    vec![
        ("not an object", json!([])),
        ("a string", json!("local/probe")),
        ("an unknown field", set("homepage", json!("x"))),
        ("$schema not a string", set("$schema", json!(1))),
        ("$schema null", set("$schema", Value::Null)),
        (
            "$schema of 2049 characters",
            set("$schema", json!("x".repeat(2049))),
        ),
        ("no manifest_version", with("manifest_version", None)),
        ("manifest_version 2", set("manifest_version", json!(2))),
        (
            "manifest_version \"1\"",
            set("manifest_version", json!("1")),
        ),
        ("no id", with("id", None)),
        ("an uppercase id", set("id", json!("local/Probe"))),
        ("an empty local name", set("id", json!("local/"))),
        ("a local name starting with -", set("id", json!("local/-a"))),
        ("a local name ending with -", set("id", json!("local/a-"))),
        ("a local name with _", set("id", json!("local/a_b"))),
        (
            "a local name of 65",
            set("id", json!(format!("local/{}", "a".repeat(65)))),
        ),
        ("another host", set("id", json!("gitlab.com/a/b"))),
        ("no repository", set("id", json!("github.com/a"))),
        (
            "an owner of 40",
            set("id", json!(format!("github.com/{}/b", "a".repeat(40)))),
        ),
        (
            "an owner ending with -",
            set("id", json!("github.com/a-/b")),
        ),
        (
            "a repository starting with .",
            set("id", json!("github.com/a/.b")),
        ),
        (
            "a repository of 101",
            set("id", json!(format!("github.com/a/{}", "b".repeat(101)))),
        ),
        (
            "a path below the repository",
            set("id", json!("github.com/a/b/c")),
        ),
        ("an id with a line break", set("id", json!("local/a\n"))),
        ("no version", with("version", None)),
        ("a version of two parts", set("version", json!("1.0"))),
        ("a leading zero", set("version", json!("01.0.0"))),
        (
            "a pre-release leading zero",
            set("version", json!("1.0.0-01")),
        ),
        ("a v prefix", set("version", json!("v1.0.0"))),
        ("an empty pre-release", set("version", json!("1.0.0-"))),
        (
            "a version of 65",
            set("version", json!(format!("1.0.0-{}", "a".repeat(59)))),
        ),
        ("no author", with("author", None)),
        ("an empty author", set("author", json!(""))),
        ("an author of 101", set("author", json!("é".repeat(101)))),
        ("a line break in the author", set("author", json!("a\nb"))),
        ("DEL in the author", set("author", json!("a\u{7f}b"))),
        (
            "a C1 control in the author",
            set("author", json!("a\u{85}b")),
        ),
        ("no permissions", with("permissions", None)),
        ("permissions not a list", set("permissions", json!({}))),
        (
            "33 permissions",
            set(
                "permissions",
                json!(vec![json!({ "name": "log", "required": true }); 33]),
            ),
        ),
        (
            "a permission as an array",
            set("permissions", json!([["log", true]])),
        ),
        (
            "an uppercase permission",
            set("permissions", permission(json!("Log"), json!(true))),
        ),
        (
            "a permission starting with a digit",
            set("permissions", permission(json!("1a"), json!(true))),
        ),
        (
            "a permission of 33",
            set(
                "permissions",
                permission(json!("a".repeat(33)), json!(true)),
            ),
        ),
        (
            "required not a boolean",
            set("permissions", permission(json!("log"), json!("yes"))),
        ),
        (
            "no required",
            set("permissions", json!([{ "name": "log" }])),
        ),
        (
            "an unknown field in a permission",
            set(
                "permissions",
                json!([{ "name": "log", "required": true, "scope": "x" }]),
            ),
        ),
        ("no login_providers", with("login_providers", None)),
        (
            "an unknown login provider",
            set("login_providers", json!(["gitlab"])),
        ),
        (
            "a login provider twice",
            set("login_providers", json!(["github", "github"])),
        ),
    ]
}

#[test]
fn the_schema_and_the_parser_accept_the_same_manifests() {
    let schema = schema();
    for (case, manifest) in valid() {
        assert!(
            valid_for_schema(&schema, &manifest),
            "the schema refused {case}"
        );
        assert_eq!(parse(&manifest), Ok(()), "the parser refused {case}");
    }
}

#[test]
fn the_schema_and_the_parser_refuse_the_same_manifests() {
    let schema = schema();
    for (case, manifest) in invalid() {
        assert!(
            !valid_for_schema(&schema, &manifest),
            "the schema accepted {case}"
        );
        assert_eq!(
            parse(&manifest),
            Err(ErrorKind::InvalidManifest),
            "the parser's verdict on {case}"
        );
    }
}

/// Rules only the host can check: the schema accepts these, the parser refuses them.
#[test]
fn the_host_also_refuses_what_a_schema_cannot_see() {
    let schema = schema();
    let host_only = [
        // The host knows only some permissions; the schema accepts any well-formed name
        (
            "an unknown permission",
            set("permissions", permission(json!("telepathy"), json!(true))),
            ErrorKind::UnknownPermission,
        ),
        // A schema cannot compare two items of a list by one of their fields
        (
            "a permission both required and optional",
            set(
                "permissions",
                json!([
                    { "name": "log", "required": true },
                    { "name": "log", "required": false }
                ]),
            ),
            ErrorKind::InvalidManifest,
        ),
        // The host compares versions as numbers, which must fit in 64 bits
        (
            "a version number above 64 bits",
            set("version", json!("18446744073709551616.0.0")),
            ErrorKind::InvalidManifest,
        ),
    ];
    for (case, manifest, kind) in host_only {
        assert!(
            valid_for_schema(&schema, &manifest),
            "the schema refused {case}"
        );
        assert_eq!(
            parse(&manifest),
            Err(kind),
            "the parser's verdict on {case}"
        );
    }

    // A JSON parser keeps one of two fields with the same name, so the schema sees one
    let duplicate = br#"{"manifest_version":1,"id":"local/a","id":"local/b","version":"1.0.0",
        "author":"A","permissions":[],"login_providers":[]}"#;
    let as_a_schema_sees_it: Value = serde_json::from_slice(duplicate).expect("JSON");
    assert!(valid_for_schema(&schema, &as_a_schema_sees_it));
    assert_eq!(
        Manifest::parse(duplicate)
            .map(drop)
            .map_err(|error| error.kind()),
        Err(ErrorKind::InvalidManifest)
    );
}

/// SC-006, FR-017: every plugin in this repository has a valid manifest with a local id.
#[test]
fn every_manifest_in_the_repository_is_valid_and_local() {
    let schema = schema();
    let mut found = Vec::new();
    for group in ["examples", "hostile", "fixtures"] {
        let dir = repository().join("plugins").join(group);
        for entry in std::fs::read_dir(&dir).expect("the plugin group exists") {
            let path = entry.expect("readable").path().join("manifest.json");
            if !path.is_file() {
                continue;
            }
            let bytes = std::fs::read(&path).expect("readable");
            let manifest: Value = serde_json::from_slice(&bytes).expect("JSON");
            assert!(
                valid_for_schema(&schema, &manifest),
                "{} breaks the schema",
                path.display()
            );
            let parsed = Manifest::parse(&bytes)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(
                parsed.id().as_str().starts_with("local/"),
                "{} has a repository id",
                path.display()
            );
            found.push(parsed.id().to_string());
        }
    }
    assert!(
        found.iter().any(|id| id == "local/hello-widget"),
        "the example's manifest was not found: {found:?}"
    );
}
