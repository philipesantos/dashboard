//! The plugin manifest: who a plugin is and what it needs (`specs/002-plugin-permissions/`,
//! data-model.md and research R2, R3).
//!
//! The manifest is untrusted input. It is parsed into typed structs, so nothing outside the
//! format is accepted, then checked against every rule of `wit/manifest.schema.json`, which
//! `tests/manifest_schema.rs` keeps in agreement with this parser.

use std::fmt;

use crate::error::{ErrorKind, RuntimeError};

/// Largest manifest accepted, checked before parsing.
pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;

/// A plugin's manifest, as checked by [`Manifest::parse`]; there is no other way to build one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    id: PluginId,
    version: String,
    author: String,
    permissions: Vec<PermissionRequest>,
    login_providers: Vec<String>,
}

/// Where a plugin comes from: `github.com/<owner>/<repository>` or `local/<name>`, lowercase.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginId(String);

/// A permission the manifest requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    /// A permission the host knows.
    pub name: String,
    /// `true`: approved with the other required permissions, or the plugin does not run.
    /// `false`: optional, off until the user turns it on.
    pub required: bool,
}

impl Manifest {
    /// Parses and checks a manifest section's payload. Refuses it as `invalid-manifest`, naming
    /// the field and the problem, or as `unknown-permission`, naming every permission the host
    /// does not know.
    pub fn parse(_payload: &[u8]) -> Result<Manifest, RuntimeError> {
        Err(RuntimeError::new(
            ErrorKind::InvalidManifest,
            "not implemented yet",
        ))
    }

    pub fn id(&self) -> &PluginId {
        &self.id
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    /// Every permission requested, once each, in the manifest's order.
    pub fn permissions(&self) -> &[PermissionRequest] {
        &self.permissions
    }

    /// The permission named `name`, if the manifest requests it.
    pub fn permission(&self, name: &str) -> Option<&PermissionRequest> {
        self.permissions
            .iter()
            .find(|permission| permission.name == name)
    }

    pub fn login_providers(&self) -> &[String] {
        &self.login_providers
    }
}

impl PluginId {
    /// Checks a plugin id; the error says which form it must take.
    pub fn parse(_id: &str) -> Result<PluginId, String> {
        Err("not implemented yet".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    /// A valid manifest, changed by `edit`.
    fn manifest_with(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut manifest = json!({
            "manifest_version": 1,
            "id": "local/probe",
            "version": "1.0.0",
            "author": "Ada Lovelace",
            "permissions": [{ "name": "log", "required": true }],
            "login_providers": []
        });
        edit(&mut manifest);
        serde_json::to_vec(&manifest).expect("a JSON value always serializes")
    }

    fn parse_with(edit: impl FnOnce(&mut Value)) -> Result<Manifest, RuntimeError> {
        Manifest::parse(&manifest_with(edit))
    }

    /// Asserts that `payload` is refused as `invalid-manifest` and returns the detail.
    fn invalid(payload: &[u8]) -> String {
        match Manifest::parse(payload) {
            Ok(manifest) => panic!(
                "accepted {:?} as {manifest:?}",
                String::from_utf8_lossy(payload)
            ),
            Err(error) => {
                assert_eq!(
                    error.kind(),
                    ErrorKind::InvalidManifest,
                    "{error} for {:?}",
                    String::from_utf8_lossy(payload)
                );
                error.detail().to_owned()
            }
        }
    }

    /// Asserts that the edited manifest is `invalid-manifest` and that its detail names `field`.
    fn assert_invalid_field(field: &str, edit: impl FnOnce(&mut Value)) {
        let detail = invalid(&manifest_with(edit));
        assert!(
            detail.contains(field),
            "the detail `{detail}` does not name `{field}`"
        );
    }

    fn set(field: &'static str, value: Value) -> impl FnOnce(&mut Value) {
        move |manifest| manifest[field] = value
    }

    #[test]
    fn parses_a_valid_manifest() {
        let manifest = parse_with(|_| {}).expect("valid");
        assert_eq!(manifest.id().as_str(), "local/probe");
        assert_eq!(manifest.id().to_string(), "local/probe");
        assert_eq!(manifest.version(), "1.0.0");
        assert_eq!(manifest.author(), "Ada Lovelace");
        assert_eq!(
            manifest.permissions(),
            [PermissionRequest {
                name: "log".to_owned(),
                required: true
            }]
        );
        assert_eq!(manifest.permission("log").map(|p| p.required), Some(true));
        assert_eq!(manifest.permission("telepathy"), None);
        assert!(manifest.login_providers().is_empty());
    }

    #[test]
    fn accepts_no_permissions_and_an_optional_one() {
        let manifest = parse_with(set("permissions", json!([]))).expect("valid");
        assert!(manifest.permissions().is_empty());

        let manifest = parse_with(set(
            "permissions",
            json!([{ "name": "log", "required": false }]),
        ))
        .expect("valid");
        assert_eq!(manifest.permission("log").map(|p| p.required), Some(false));
    }

    #[test]
    fn accepts_and_ignores_schema() {
        let manifest =
            parse_with(set("$schema", json!("../../../wit/manifest.schema.json"))).expect("valid");
        assert_eq!(manifest, parse_with(|_| {}).expect("valid"));
        parse_with(set("$schema", json!("x".repeat(2048)))).expect("2048 characters");
        assert_invalid_field("$schema", set("$schema", json!("x".repeat(2049))));
        assert_invalid_field("$schema", set("$schema", json!(1)));
    }

    #[test]
    fn manifest_version_must_be_1() {
        // JSON Schema compares numbers by value, so 1.0 is 1
        parse_with(set("manifest_version", json!(1.0))).expect("1.0 is 1");
        for value in [json!(2), json!(0), json!("1"), json!(null), json!(1.5)] {
            assert_invalid_field("manifest_version", set("manifest_version", value));
        }
        assert_invalid_field("manifest_version", |manifest| {
            manifest
                .as_object_mut()
                .expect("an object")
                .remove("manifest_version");
        });
    }

    #[test]
    fn accepts_both_id_forms() {
        let ids = [
            "local/a".to_owned(),
            "local/hello-widget".to_owned(),
            "local/a--b".to_owned(),
            format!("local/{}", "a".repeat(64)),
            "github.com/a/b".to_owned(),
            "github.com/owner-name/repo.name_x-y".to_owned(),
            "github.com/0/_".to_owned(),
            "github.com/a/-".to_owned(),
            "github.com/a/a..b".to_owned(),
            format!("github.com/{}/b", "a".repeat(39)),
            format!("github.com/a/{}", "b".repeat(100)),
        ];
        for id in ids {
            let manifest = parse_with(set("id", json!(id))).unwrap_or_else(|error| {
                panic!("{id} refused: {error}");
            });
            assert_eq!(manifest.id().as_str(), id);
            assert_eq!(PluginId::parse(&id).map(|id| id.to_string()), Ok(id));
        }
    }

    #[test]
    fn refuses_every_other_id() {
        let ids = [
            String::new(),
            "x".to_owned(),
            "local".to_owned(),
            "local/".to_owned(),
            "Local/a".to_owned(),
            "local/A".to_owned(),
            "local/-a".to_owned(),
            "local/a-".to_owned(),
            "local/a_b".to_owned(),
            "local/a.b".to_owned(),
            "local/a/b".to_owned(),
            "local/a\n".to_owned(),
            " local/a".to_owned(),
            "local/é".to_owned(),
            format!("local/{}", "a".repeat(65)),
            "github.com/a".to_owned(),
            "github.com/a/".to_owned(),
            "github.com//b".to_owned(),
            "github.com/-a/b".to_owned(),
            "github.com/a-/b".to_owned(),
            "github.com/a_b/c".to_owned(),
            "github.com/a.b/c".to_owned(),
            format!("github.com/{}/b", "a".repeat(40)),
            "github.com/a/.b".to_owned(),
            "github.com/a/..".to_owned(),
            "github.com/a/.".to_owned(),
            format!("github.com/a/{}", "b".repeat(101)),
            "github.com/a/B".to_owned(),
            "github.com/A/b".to_owned(),
            "GitHub.com/a/b".to_owned(),
            "github.com/a/b/c".to_owned(),
            "gitlab.com/a/b".to_owned(),
            "www.github.com/a/b".to_owned(),
            "https://github.com/a/b".to_owned(),
        ];
        for id in ids {
            assert_invalid_field("id", set("id", json!(id)));
            assert!(PluginId::parse(&id).is_err(), "{id:?} accepted");
        }
        assert_invalid_field("id", set("id", json!(7)));
    }

    #[test]
    fn version_follows_semver() {
        let valid = [
            "0.1.0".to_owned(),
            "1.2.3-alpha.1+build.5".to_owned(),
            "1.0.0-0.3.7".to_owned(),
            "1.0.0-x-y-z.--".to_owned(),
            "1.0.0+001".to_owned(),
            "1.0.0-alpha+20130313144700".to_owned(),
            format!("1.0.0-{}", "a".repeat(58)),
        ];
        for version in valid {
            let manifest = parse_with(set("version", json!(version)))
                .unwrap_or_else(|error| panic!("{version} refused: {error}"));
            assert_eq!(manifest.version(), version);
        }
        let invalid_versions = [
            String::new(),
            "1".to_owned(),
            "1.0".to_owned(),
            "01.0.0".to_owned(),
            "1.00.0".to_owned(),
            "1.0.0-01".to_owned(),
            "v1.0.0".to_owned(),
            "1.0.0-".to_owned(),
            "1.0.0+".to_owned(),
            "1.0.0-a..b".to_owned(),
            "1.0.0-é".to_owned(),
            "1.0.0 ".to_owned(),
            "1.0.0\n".to_owned(),
            format!("1.0.0-{}", "a".repeat(59)),
        ];
        for version in invalid_versions {
            assert_invalid_field("version", set("version", json!(version)));
        }
    }

    #[test]
    fn author_has_1_to_100_characters_and_no_control_character() {
        for author in [
            "A".to_owned(),
            "é".repeat(100),
            "Dashboard contributors".to_owned(),
            // Bidirectional controls are not control characters; every display escapes them
            "\u{202e}dlrow".to_owned(),
        ] {
            let manifest = parse_with(set("author", json!(author)))
                .unwrap_or_else(|error| panic!("{author:?} refused: {error}"));
            assert_eq!(manifest.author(), author);
        }
        for author in [
            String::new(),
            "é".repeat(101),
            "a\nb".to_owned(),
            "a\tb".to_owned(),
            "a\u{0}b".to_owned(),
            "a\u{1b}[2Kb".to_owned(),
            "a\u{7f}b".to_owned(),
            "a\u{85}b".to_owned(),
            "a\u{9f}b".to_owned(),
        ] {
            assert_invalid_field("author", set("author", json!(author)));
        }
    }

    #[test]
    fn permissions_hold_at_most_32_entries() {
        let entry = json!({ "name": "log", "required": true });
        parse_with(set("permissions", json!(vec![entry.clone(); 32]))).expect("32 entries");
        assert_invalid_field("permissions", set("permissions", json!(vec![entry; 33])));
        assert_invalid_field("permissions", set("permissions", json!({})));
    }

    #[test]
    fn a_permission_has_a_well_formed_name_and_required() {
        for name in [
            json!("Log"),
            json!("1log"),
            json!("-log"),
            json!("log_x"),
            json!("log.x"),
            json!(""),
            json!("a".repeat(33)),
            json!(1),
        ] {
            assert_invalid_field(
                "permissions[0].name",
                set("permissions", json!([{ "name": name, "required": true }])),
            );
        }
        assert_invalid_field(
            "permissions[0].required",
            set("permissions", json!([{ "name": "log", "required": "yes" }])),
        );
        assert_invalid_field("required", set("permissions", json!([{ "name": "log" }])));
        assert_invalid_field(
            "scope",
            set(
                "permissions",
                json!([{ "name": "log", "required": true, "scope": "all" }]),
            ),
        );
        assert_invalid_field("permissions[0]", set("permissions", json!(["log"])));
    }

    #[test]
    fn the_same_permission_twice_counts_once() {
        let manifest = parse_with(set(
            "permissions",
            json!([
                { "name": "log", "required": false },
                { "name": "log", "required": false }
            ]),
        ))
        .expect("valid");
        assert_eq!(
            manifest.permissions(),
            [PermissionRequest {
                name: "log".to_owned(),
                required: false
            }]
        );
    }

    #[test]
    fn a_permission_both_required_and_optional_is_invalid() {
        let detail = invalid(&manifest_with(set(
            "permissions",
            json!([
                { "name": "log", "required": true },
                { "name": "log", "required": false }
            ]),
        )));
        assert!(detail.contains("permissions"), "{detail}");
        assert!(detail.contains("log"), "{detail}");
    }

    #[test]
    fn an_unknown_permission_is_named() {
        let error = parse_with(set(
            "permissions",
            json!([
                { "name": "telepathy", "required": true },
                { "name": "log", "required": true },
                { "name": "time-travel", "required": false }
            ]),
        ))
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UnknownPermission);
        assert_eq!(error.names(), ["telepathy", "time-travel"]);
    }

    #[test]
    fn schema_rules_come_before_unknown_permissions() {
        let error = parse_with(|manifest| {
            manifest["permissions"] = json!([{ "name": "telepathy", "required": true }]);
            manifest["author"] = json!("");
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidManifest);
    }

    #[test]
    fn login_providers_are_unique_and_github_only() {
        let manifest = parse_with(set("login_providers", json!(["github"]))).expect("valid");
        assert_eq!(manifest.login_providers(), ["github"]);
        assert_invalid_field(
            "login_providers",
            set("login_providers", json!(["github", "github"])),
        );
        assert_invalid_field(
            "login_providers[0]",
            set("login_providers", json!(["gitlab"])),
        );
        assert_invalid_field(
            "login_providers",
            set("login_providers", json!(vec!["github"; 9])),
        );
        assert_invalid_field("login_providers", set("login_providers", json!("github")));
    }

    #[test]
    fn every_field_is_required() {
        for field in [
            "manifest_version",
            "id",
            "version",
            "author",
            "permissions",
            "login_providers",
        ] {
            assert_invalid_field(field, |manifest| {
                manifest.as_object_mut().expect("an object").remove(field);
            });
        }
    }

    #[test]
    fn refuses_unknown_and_duplicate_fields() {
        assert_invalid_field("homepage", set("homepage", json!("https://example.com")));

        let detail = invalid(
            br#"{"manifest_version":1,"id":"local/a","id":"local/b","version":"1.0.0",
                "author":"A","permissions":[],"login_providers":[]}"#,
        );
        assert!(detail.contains("id"), "{detail}");
    }

    #[test]
    fn escapes_what_the_detail_quotes() {
        let detail = invalid(&manifest_with(set("x\nerror: forged", json!(1))));
        assert!(!detail.contains('\n'), "{detail}");
        assert!(detail.contains(r"x\nerror: forged"), "{detail}");
    }

    #[test]
    fn refuses_a_payload_over_16_kib_before_parsing() {
        let mut payload = manifest_with(|_| {});
        payload.resize(MAX_MANIFEST_BYTES, b' ');
        Manifest::parse(&payload).expect("exactly 16 KiB, padded with spaces");

        payload.push(b' ');
        let detail = invalid(&payload);
        assert!(detail.contains("16384"), "{detail}");
    }

    #[test]
    fn refuses_a_payload_that_is_not_utf8() {
        let mut payload = manifest_with(set("author", json!("Ada")));
        let at = payload
            .windows(3)
            .position(|window| window == b"Ada")
            .expect("the author is in the payload");
        payload[at] = 0xff;
        let detail = invalid(&payload);
        assert!(detail.contains("UTF-8"), "{detail}");
    }

    #[test]
    fn refuses_a_payload_that_is_not_a_json_object() {
        for payload in [
            &b""[..],
            b"   ",
            b"not json",
            b"{",
            b"[]",
            b"\"local/probe\"",
            b"null",
            b"{} {}",
            b"\xef\xbb\xbf{}",
        ] {
            invalid(payload);
        }
        let mut trailing = manifest_with(|_| {});
        trailing.extend_from_slice(b" []");
        invalid(&trailing);
    }

    #[test]
    fn refuses_deep_nesting_without_overflowing_the_stack() {
        // Left open, so the payload stays under the size cap and reaches the parser
        let depth = 10_000;
        invalid("[".repeat(depth).as_bytes());
        invalid(r#"{"a":"#.repeat(depth / 5).as_bytes());
        invalid(format!(r#"{{"manifest_version":1,"id":{}"#, "[".repeat(depth)).as_bytes());
        invalid(format!(r#"{{"$schema":{}"#, "[".repeat(depth)).as_bytes());
    }
}
