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

// Feature 003: the connector schema, `wit/connector.schema.json`, and the host's connector
// parser agree on every connector of the corpus below; the rules only the host checks are
// listed apart (`specs/003-plugin-connectors/tasks.md`, T052).

mod connectors {
    use boon::{Compiler, SchemaIndex, Schemas};
    use runtime::connector::{Connector, Source};
    use serde_json::{Value, json};

    use super::repository;

    fn schema() -> (Schemas, SchemaIndex) {
        let path = repository().join("wit").join("connector.schema.json");
        let mut schemas = Schemas::new();
        let index = Compiler::new()
            .compile(path.to_str().expect("a UTF-8 path"), &mut schemas)
            .unwrap_or_else(|error| panic!("the connector schema compiles: {error:#}"));
        (schemas, index)
    }

    fn valid_for_schema(schema: &(Schemas, SchemaIndex), connector: &Value) -> bool {
        schema.0.validate(connector, schema.1).is_ok()
    }

    fn parses(connector: &Value, source: Source) -> bool {
        Connector::parse(&serde_json::to_vec(connector).unwrap(), source).is_ok()
    }

    /// A valid local connector, edited.
    fn local(edit: impl FnOnce(&mut Value)) -> Value {
        let mut connector = json!({
            "connector_version": 1,
            "id": "local/weather",
            "version": "0.1.0",
            "author": "Ada Lovelace",
            "description": "Weather forecasts",
            "destinations": { "api": "https://api.weather.test" },
            "operations": {
                "forecast": {
                    "kind": "read", "method": "GET", "destination": "api",
                    "path": "/v1/{city}/forecast",
                    "query": { "days": "{days}", "units": "metric" },
                    "headers": { "Accept": "application/json" },
                    "parameters": {
                        "city": { "type": "text", "max_length": 64, "pattern": "[a-z]+" },
                        "days": { "type": "integer", "minimum": 1, "maximum": 7, "required": false }
                    },
                    "response": { "format": "json", "headers": ["ETag"] }
                }
            }
        });
        edit(&mut connector);
        connector
    }

    fn catalog(edit: impl FnOnce(&mut Value)) -> Value {
        local(|c| {
            c["id"] = json!("catalog/weather");
            c["reserves"] = json!(["weather.test"]);
            edit(c);
        })
    }

    fn op(c: &mut Value) -> &mut Value {
        &mut c["operations"]["forecast"]
    }

    type Edit = fn(&mut Value);

    /// Local connectors both must accept.
    fn valid_local() -> Vec<(&'static str, Edit)> {
        vec![
            ("the base connector", |_| {}),
            ("$schema", |c| {
                c["$schema"] = json!("../../wit/connector.schema.json")
            }),
            ("a 64-character name", |c| {
                c["id"] = json!(format!("local/{}", "a".repeat(64)))
            }),
            ("a pre-release version", |c| {
                c["version"] = json!("1.0.0-beta.1")
            }),
            ("100-character author", |c| {
                c["author"] = json!("a".repeat(100))
            }),
            ("a port", |c| {
                c["destinations"]["api"] = json!("https://api.weather.test:8443")
            }),
            ("a root path", |c| {
                c["destinations"]["api"] = json!("https://api.weather.test/")
            }),
            ("an uppercase host", |c| {
                c["destinations"]["api"] = json!("https://API.Weather.test")
            }),
            ("loopback over HTTP", |c| {
                c["destinations"]["api"] = json!("http://127.0.0.1:8765")
            }),
            ("localhost over HTTP", |c| {
                c["destinations"]["api"] = json!("http://localhost:8765")
            }),
            ("[::1] over HTTP", |c| {
                c["destinations"]["api"] = json!("http://[::1]:8765")
            }),
            ("HEAD", |c| op(c)["method"] = json!("HEAD")),
            ("a change with a body", |c| {
                op(c)["kind"] = json!("change");
                op(c)["method"] = json!("POST");
                op(c)["query"] = json!({ "days": "{days}" });
                op(c)["body"] = json!({ "units": "metric" });
            }),
            (
                "values",
                |c| {
                    op(c)["parameters"]["city"] =
                        json!({ "type": "text", "max_length": 8, "values": ["rome", "paris"] })
                },
            ),
            ("a flag", |c| {
                op(c)["parameters"]["metric"] = json!({ "type": "flag", "required": false });
                op(c)["query"]["metric"] = json!("{metric}");
            }),
            ("text format", |c| {
                op(c)["response"] = json!({ "format": "text" })
            }),
            ("eight response headers", |c| {
                op(c)["response"]["headers"] = json!(["A", "B", "C", "D", "E", "F", "G", "H"])
            }),
            ("Accept-Language", |c| {
                op(c)["headers"]["Accept-Language"] = json!("pt-BR")
            }),
        ]
    }

    /// Connectors both must refuse, whatever the source.
    fn invalid() -> Vec<(&'static str, Edit)> {
        vec![
            ("an unknown field", |c| c["colour"] = json!("blue")),
            ("an unknown operation field", |c| {
                op(c)["timeout"] = json!(5)
            }),
            ("connector_version 2", |c| c["connector_version"] = json!(2)),
            ("a malformed id", |c| c["id"] = json!("Local/Weather")),
            ("a 65-character name", |c| {
                c["id"] = json!(format!("local/{}", "a".repeat(65)))
            }),
            ("a version that is not SemVer", |c| {
                c["version"] = json!("1.0")
            }),
            ("an empty author", |c| c["author"] = json!("")),
            ("a 201-character description", |c| {
                c["description"] = json!("d".repeat(201))
            }),
            ("a control character", |c| c["author"] = json!("A\nB")),
            ("no destination", |c| c["destinations"] = json!({})),
            ("a bad destination name", |c| {
                c["destinations"]["Bad"] = json!("https://x.test")
            }),
            ("a destination with a path", |c| {
                c["destinations"]["api"] = json!("https://api.weather.test/v1")
            }),
            ("a destination with a query", |c| {
                c["destinations"]["api"] = json!("https://api.weather.test?a=1")
            }),
            ("a destination with a user", |c| {
                c["destinations"]["api"] = json!("https://ada@api.weather.test")
            }),
            ("an uppercase scheme", |c| {
                c["destinations"]["api"] = json!("HTTPS://api.weather.test")
            }),
            ("plain HTTP elsewhere", |c| {
                c["destinations"]["api"] = json!("http://192.168.1.10:8765")
            }),
            ("HTTP localhost without a port", |c| {
                c["destinations"]["api"] = json!("http://localhost")
            }),
            ("no operation", |c| c["operations"] = json!({})),
            ("a bad kind", |c| op(c)["kind"] = json!("write")),
            ("a bad method", |c| op(c)["method"] = json!("OPTIONS")),
            ("a relative path", |c| op(c)["path"] = json!("v1/{city}")),
            ("a mixed path segment", |c| {
                op(c)["path"] = json!("/v1/x{city}")
            }),
            ("a 17th query pair", |c| {
                for i in 0..16 {
                    op(c)["query"][format!("q{i}")] = json!("x");
                }
            }),
            ("a bad query name", |c| op(c)["query"]["a b"] = json!("x")),
            ("a 257-character query value", |c| {
                op(c)["query"]["units"] = json!("u".repeat(257))
            }),
            ("a forbidden header", |c| {
                op(c)["headers"]["Authorization"] = json!("x")
            }),
            ("a header value with a line break", |c| {
                op(c)["headers"]["Accept"] = json!("a\r\nb")
            }),
            ("a body that is not an object", |c| {
                op(c)["kind"] = json!("change");
                op(c)["body"] = json!([1]);
            }),
            ("a parameter without a type", |c| {
                op(c)["parameters"]["city"] = json!({ "max_length": 8 })
            }),
            ("text without max_length", |c| {
                op(c)["parameters"]["city"] = json!({ "type": "text" })
            }),
            ("max_length 2049", |c| {
                op(c)["parameters"]["city"] = json!({ "type": "text", "max_length": 2049 })
            }),
            (
                "pattern and values",
                |c| {
                    op(c)["parameters"]["city"] =
                        json!({ "type": "text", "max_length": 8, "pattern": "a", "values": ["a"] })
                },
            ),
            (
                "a 513-character pattern",
                |c| {
                    op(c)["parameters"]["city"] =
                        json!({ "type": "text", "max_length": 8, "pattern": "a".repeat(513) })
                },
            ),
            (
                "repeated values",
                |c| {
                    op(c)["parameters"]["city"] =
                        json!({ "type": "text", "max_length": 8, "values": ["a", "a"] })
                },
            ),
            ("an integer without maximum", |c| {
                op(c)["parameters"]["days"] =
                    json!({ "type": "integer", "minimum": 1, "required": false })
            }),
            ("a bad parameter name", |c| {
                op(c)["parameters"]["City"] = json!({ "type": "flag" })
            }),
            ("a bad format", |c| {
                op(c)["response"]["format"] = json!("xml")
            }),
            ("Set-Cookie passed on", |c| {
                op(c)["response"]["headers"] = json!(["Set-Cookie"])
            }),
            ("a repeated response header", |c| {
                op(c)["response"]["headers"] = json!(["ETag", "ETag"])
            }),
            ("a credential", |c| c["credential"] = json!("token")),
        ]
    }

    /// Rules only the host checks, which the schema cannot express: the schema accepts these
    /// local connectors, and the parser refuses them.
    fn host_only_local() -> Vec<(&'static str, Edit)> {
        vec![
            ("an X- header outside the catalog", |c| {
                op(c)["headers"]["X-HTTP-Method-Override"] = json!("DELETE")
            }),
            ("reserves outside the catalog", |c| {
                c["reserves"] = json!(["weather.test"])
            }),
            ("a destination ending with a dot", |c| {
                c["destinations"]["api"] = json!("https://api.weather.test.")
            }),
            ("an IP address over HTTPS", |c| {
                c["destinations"]["api"] = json!("https://10.0.0.1")
            }),
            ("a catalog id outside the catalog", |c| {
                c["id"] = json!("catalog/weather")
            }),
            ("a placeholder naming no parameter", |c| {
                op(c)["path"] = json!("/v1/{town}/forecast")
            }),
            ("a parameter used nowhere", |c| {
                op(c)["parameters"]["unused"] = json!({ "type": "flag" })
            }),
            ("a parameter used twice", |c| {
                op(c)["query"]["again"] = json!("{city}")
            }),
            ("an optional parameter in the path", |c| {
                op(c)["parameters"]["city"]["required"] = json!(false)
            }),
            ("a POST read outside the catalog", |c| {
                op(c)["method"] = json!("POST")
            }),
            ("a read with a body outside the catalog", |c| {
                op(c)["body"] = json!({ "q": "x" })
            }),
            (
                "minimum greater than maximum",
                |c| {
                    op(c)["parameters"]["days"] =
                        json!({ "type": "integer", "minimum": 8, "maximum": 1, "required": false })
                },
            ),
            ("a destination that is not a key", |c| {
                op(c)["destination"] = json!("elsewhere")
            }),
            ("a body over 4 KiB", |c| {
                op(c)["kind"] = json!("change");
                op(c)["body"] = json!({ "pad": "x".repeat(4096) });
            }),
            ("a file over 64 KiB", |c| {
                c["description"] = json!("d".repeat(200))
            }),
            ("a pattern that does not compile", |c| {
                op(c)["parameters"]["city"]["pattern"] = json!("(")
            }),
        ]
    }

    #[test]
    fn the_schema_and_the_parser_accept_the_same_connectors() {
        let schema = schema();
        for (case, edit) in valid_local() {
            let connector = local(edit);
            assert!(
                valid_for_schema(&schema, &connector),
                "{case}: the schema refuses it"
            );
            assert!(
                parses(&connector, Source::Local),
                "{case}: the parser refuses it"
            );
        }
        let connector = catalog(|c| op(c)["headers"]["X-Api-Version"] = json!("2"));
        assert!(valid_for_schema(&schema, &connector));
        assert!(parses(&connector, Source::Catalog));
    }

    #[test]
    fn the_schema_and_the_parser_refuse_the_same_connectors() {
        let schema = schema();
        for (case, edit) in invalid() {
            let connector = local(edit);
            assert!(
                !valid_for_schema(&schema, &connector),
                "{case}: the schema accepts it"
            );
            assert!(
                !parses(&connector, Source::Local),
                "{case}: the parser accepts it"
            );
        }
    }

    #[test]
    fn the_host_also_refuses_what_a_schema_cannot_see() {
        let schema = schema();
        for (case, edit) in host_only_local() {
            let mut connector = local(edit);
            if case == "a file over 64 KiB" {
                // Within every per-field limit, over the file's
                let query: serde_json::Map<String, Value> = (0..16)
                    .map(|i| (format!("q{i}"), json!("v".repeat(256))))
                    .collect();
                for i in 0..63 {
                    connector["operations"][format!("op{i}")] = json!({
                        "kind": "read", "method": "GET", "destination": "api", "path": "/",
                        "query": query, "response": { "format": "json" }
                    });
                }
                let bytes = serde_json::to_vec(&connector).unwrap();
                assert!(bytes.len() > runtime::connector::MAX_CONNECTOR_BYTES);
                assert!(
                    valid_for_schema(&schema, &connector),
                    "{case}: the schema refuses it"
                );
                assert!(Connector::parse(&bytes, Source::Local).is_err(), "{case}");
                continue;
            }
            assert!(
                valid_for_schema(&schema, &connector),
                "{case}: the schema refuses it, so it is no host-only rule"
            );
            assert!(
                !parses(&connector, Source::Local),
                "{case}: the parser accepts it"
            );
        }
        // A catalog connector without reserves, or with plain HTTP
        for (case, connector) in [
            (
                "reserves missing in the catalog",
                catalog(|c| {
                    c.as_object_mut().unwrap().remove("reserves");
                }),
            ),
            (
                "plain HTTP in the catalog",
                catalog(|c| c["destinations"]["api"] = json!("http://localhost:8765")),
            ),
        ] {
            assert!(valid_for_schema(&schema, &connector), "{case}");
            assert!(!parses(&connector, Source::Catalog), "{case}");
        }
    }

    #[test]
    fn every_catalog_connector_in_the_repository_validates_and_parses() {
        let schema = schema();
        let dir = repository().join("connectors").join("catalog");
        let mut found = 0;
        for entry in std::fs::read_dir(&dir).expect("the catalog directory") {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            let connector: Value = serde_json::from_slice(&bytes).unwrap();
            assert!(
                valid_for_schema(&schema, &connector),
                "{} breaks the schema",
                path.display()
            );
            let parsed = Connector::parse(&bytes, Source::Catalog)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let name = path.file_stem().unwrap().to_str().unwrap();
            assert_eq!(parsed.id(), format!("catalog/{name}"));
            found += 1;
        }
        assert!(found > 0, "the catalog is empty");
    }
}

/// Manifests with `connectors`: the schema and the parser agree, the rule of feature 002 for the
/// new field.
#[test]
fn the_schema_and_the_parser_agree_on_connectors() {
    let schema = schema();
    let entry = |id: Value, major: Value, operations: Value, required: Value| {
        set(
            "connectors",
            json!([{ "id": id, "major": major, "operations": operations, "required": required }]),
        )
    };
    for (case, manifest) in [
        ("none", set("connectors", json!([]))),
        (
            "catalog",
            entry(
                json!("catalog/github"),
                json!(1),
                json!(["list-releases"]),
                json!(true),
            ),
        ),
        (
            "local",
            entry(
                json!("local/hello"),
                json!(0),
                json!(["get", "put"]),
                json!(false),
            ),
        ),
        (
            "repository",
            entry(
                json!("github.com/ada/weather"),
                json!(65535),
                json!(["forecast"]),
                json!(true),
            ),
        ),
    ] {
        assert!(
            valid_for_schema(&schema, &manifest),
            "{case}: the schema refuses it"
        );
        assert_eq!(parse(&manifest), Ok(()), "{case}: the parser refuses it");
    }
    let many: Vec<Value> = (0..17)
        .map(|i| json!({ "id": format!("local/c{i}"), "major": 1, "operations": ["get"], "required": true }))
        .collect();
    for (case, manifest) in [
        ("null", set("connectors", Value::Null)),
        ("17 connectors", set("connectors", Value::Array(many))),
        (
            "a bad id",
            entry(json!("github"), json!(1), json!(["get"]), json!(true)),
        ),
        (
            "major 65536",
            entry(json!("local/a"), json!(65536), json!(["get"]), json!(true)),
        ),
        (
            "a negative major",
            entry(json!("local/a"), json!(-1), json!(["get"]), json!(true)),
        ),
        (
            "no operation",
            entry(json!("local/a"), json!(1), json!([]), json!(true)),
        ),
        (
            "a repeated operation",
            entry(
                json!("local/a"),
                json!(1),
                json!(["get", "get"]),
                json!(true),
            ),
        ),
        (
            "a bad operation",
            entry(json!("local/a"), json!(1), json!(["Get"]), json!(true)),
        ),
        (
            "required as text",
            entry(json!("local/a"), json!(1), json!(["get"]), json!("yes")),
        ),
        (
            "an unknown field",
            set(
                "connectors",
                json!([{ "id": "local/a", "major": 1, "operations": ["get"], "required": true, "x": 1 }]),
            ),
        ),
    ] {
        assert!(
            !valid_for_schema(&schema, &manifest),
            "{case}: the schema accepts it"
        );
        assert_eq!(parse(&manifest), Err(ErrorKind::InvalidManifest), "{case}");
    }
    // Only the host sees a connector declared twice
    let twice = set(
        "connectors",
        json!([
            { "id": "local/a", "major": 1, "operations": ["get"], "required": true },
            { "id": "local/a", "major": 2, "operations": ["put"], "required": false }
        ]),
    );
    assert!(valid_for_schema(&schema, &twice));
    assert_eq!(parse(&twice), Err(ErrorKind::InvalidManifest));
}

/// Feature 004: `quota_bytes` on `storage`. The schema and the parser agree on every rule of
/// the field; the host's own cap, which may be lower than the schema's maximum, is checked at
/// load instead (`runtime/tests/permissions.rs`, the host cap case).
#[test]
fn the_schema_and_the_parser_agree_on_the_storage_quota() {
    let schema = schema();
    let storage = |entry: Value| {
        set(
            "permissions",
            json!([{ "name": "log", "required": true }, entry]),
        )
    };
    let valid = [
        (
            "the smallest quota",
            storage(json!({ "name": "storage", "required": true, "quota_bytes": 64 })),
        ),
        (
            "a quota of 64 KiB",
            storage(json!({ "name": "storage", "required": false, "quota_bytes": 65536 })),
        ),
        (
            "the largest quota",
            storage(json!({ "name": "storage", "required": true, "quota_bytes": 8_388_608 })),
        ),
        (
            "a quota written 4096.0",
            storage(json!({ "name": "storage", "required": true, "quota_bytes": 4096.0 })),
        ),
    ];
    for (case, manifest) in valid {
        assert!(
            valid_for_schema(&schema, &manifest),
            "the schema refused {case}"
        );
        assert_eq!(parse(&manifest), Ok(()), "the parser refused {case}");
    }
    let invalid = [
        (
            "storage without a quota",
            storage(json!({ "name": "storage", "required": true })),
        ),
        (
            "a quota on another permission",
            set(
                "permissions",
                json!([{ "name": "log", "required": true, "quota_bytes": 4096 }]),
            ),
        ),
        (
            "a quota of 63",
            storage(json!({ "name": "storage", "required": true, "quota_bytes": 63 })),
        ),
        (
            "a quota of 8388609",
            storage(json!({ "name": "storage", "required": true, "quota_bytes": 8_388_609 })),
        ),
        (
            "a fraction",
            storage(json!({ "name": "storage", "required": true, "quota_bytes": 64.5 })),
        ),
        (
            "a string",
            storage(json!({ "name": "storage", "required": true, "quota_bytes": "65536" })),
        ),
        (
            "null",
            storage(json!({ "name": "storage", "required": true, "quota_bytes": null })),
        ),
        (
            "two storage entries",
            set(
                "permissions",
                json!([
                    { "name": "storage", "required": true, "quota_bytes": 4096 },
                    { "name": "storage", "required": false, "quota_bytes": 4096 }
                ]),
            ),
        ),
    ];
    for (case, manifest) in invalid {
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
    // The example declares storage as authors are told to, and both accept it (SC-008)
    let example: Value = serde_json::from_slice(
        &std::fs::read(repository().join("plugins/examples/update-counter/manifest.json")).unwrap(),
    )
    .unwrap();
    assert!(valid_for_schema(&schema, &example));
    assert_eq!(parse(&example), Ok(()));
    assert_eq!(example["permissions"][0]["name"], "storage");
}
