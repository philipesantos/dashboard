//! The app's widget host, below the page (specs/009-declarative-widget-ui/research.md, R13): its
//! records stay apart from development, it embeds only what it should, it approves only bundled
//! packages that declare nothing, and its commands refuse in the order of
//! `contracts/window-commands.md` without calling the plugin.
//!
//! The packages come from `target/plugins/` (`cargo xtask plugins`).
//!
//! Part of the security core (constitution, Principle IX).

use std::path::{Path, PathBuf};
use std::time::Duration;

use dashboard_lib::name::drawn_name;
use dashboard_lib::widgets::{self, Bundled, Rendered, Widgets};
use runtime::{Limits, Runtime, RuntimeOptions};
use serde_json::{Value, json};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A package `cargo xtask plugins` built.
fn package(name: &str) -> Vec<u8> {
    let path = crate_dir()
        .join("../../target/plugins")
        .join(format!("{name}.wasm"));
    std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}; run `cargo xtask plugins` first",
            path.display()
        )
    })
}

/// A fixture built without a manifest, packed with `manifest`.
fn packed(fixture: &str, manifest: &Value) -> Vec<u8> {
    runtime::package::pack(&package(fixture), manifest.to_string().as_bytes())
        .unwrap_or_else(|error| panic!("packing {fixture}: {error}"))
}

/// A runtime with the app's options under a fresh directory.
fn app_runtime(dir: &Path, limits: Limits) -> Runtime {
    Runtime::new(limits, widgets::app_options(dir)).expect("the runtime starts")
}

fn start(dir: &Path, packages: &[(&str, Vec<u8>)]) -> Widgets {
    start_on(app_runtime(dir, Limits::default()), packages)
}

fn start_on(runtime: Runtime, packages: &[(&str, Vec<u8>)]) -> Widgets {
    let bundled: Vec<Bundled<'_>> = packages
        .iter()
        .map(|(file, bytes)| Bundled { file, bytes })
        .collect();
    Widgets::start(runtime, &bundled)
}

fn showcase() -> (&'static str, Vec<u8>) {
    ("showcase.wasm", package("showcase"))
}

fn values(pairs: &[(&str, &str)]) -> Value {
    Value::Object(
        pairs
            .iter()
            .map(|(id, text)| ((*id).to_owned(), json!(text)))
            .collect(),
    )
}

fn refused(rendered: &Rendered) -> Option<&str> {
    match rendered {
        Rendered::Refused(kind) => Some(kind),
        _ => None,
    }
}

/// The content as JSON, to look for the texts the showcase draws.
fn content_text(rendered: &Rendered) -> String {
    match rendered {
        Rendered::Content(content) => serde_json::to_string(content).unwrap(),
        other => panic!("expected content, got {other:?}"),
    }
}

/// The plugin's log in the app, as one text.
fn log_text(widgets: &Widgets, handle: &str) -> String {
    widgets
        .log(handle)
        .iter()
        .map(|entry| format!("{} {} {}\n", entry.level, entry.source, entry.message))
        .collect()
}

// FR-024: the app's records stay apart from development (research R3)

#[test]
fn the_apps_options_are_not_the_development_ones() {
    let dir = tempfile::tempdir().unwrap();
    let options = widgets::app_options(dir.path());
    assert!(!options.is_development(), "development mode is off");
    assert_eq!(options.data_dir(), dir.path().join("host"));
    if let Some(development) = RuntimeOptions::default_development_data_dir() {
        assert_ne!(options.data_dir(), development);
    }
    let development_service = RuntimeOptions::new(dir.path()).vault_service();
    assert_eq!(options.vault_service(), widgets::APP_VAULT_SERVICE);
    assert_ne!(options.vault_service(), development_service);
}

/// The test above cannot see a stray call elsewhere, so the app's source never names the
/// development records' functions.
#[test]
fn the_apps_source_never_names_the_development_records() {
    let mut stack = vec![crate_dir().join("src")];
    let mut found = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for name in ["default_development_path", "default_development_data_dir"] {
                    if text.contains(name) {
                        found.push(format!("{} names {name}", path.display()));
                    }
                }
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

// R1: which packages a build embeds

/// Two frames drawing the same name, ignoring case, each show their plugin's id beside it; a
/// name no other frame draws shows none.
#[test]
fn frames_of_one_name_show_their_ids() {
    let dir = tempfile::tempdir().unwrap();
    let impostor = packed(
        "action-probe",
        &json!({
            "manifest_version": 1,
            "id": "local/impostor",
            "version": "0.1.0",
            "author": "Someone",
            "name": "VITRINE",
            "permissions": [],
            "actions": [{ "name": "remember" }]
        }),
    );
    let alone = packed(
        "action-probe",
        &json!({
            "manifest_version": 1,
            "id": "local/alone",
            "version": "0.1.0",
            "author": "Someone",
            "name": "Outro",
            "permissions": [],
            "actions": [{ "name": "remember" }]
        }),
    );
    let widgets = start(
        dir.path(),
        &[
            showcase(),
            ("impostor.wasm", impostor),
            ("alone.wasm", alone),
        ],
    );
    let ids: Vec<(String, Option<String>)> = widgets
        .list()
        .into_iter()
        .map(|frame| (frame.name, frame.id))
        .collect();
    assert_eq!(
        ids,
        [
            ("Vitrine".to_owned(), Some("local/showcase".to_owned())),
            ("VITRINE".to_owned(), Some("local/impostor".to_owned())),
            ("Outro".to_owned(), None),
        ]
    );
    // One showcase alone has none
    assert_eq!(start(dir.path(), &[showcase()]).list()[0].id, None);
}

#[test]
fn a_build_embeds_exactly_its_packages() {
    let mut embedded: Vec<&str> = widgets::BUNDLED
        .iter()
        .map(|bundled| bundled.file)
        .collect();
    embedded.sort_unstable();
    #[cfg(not(feature = "shell-probe"))]
    let expected = vec!["showcase.wasm"];
    #[cfg(feature = "shell-probe")]
    let expected = vec![
        "showcase.wasm",
        "ui-badmanifest.wasm",
        "ui-crash.wasm",
        "ui-dialog.wasm",
        "ui-header.wasm",
        "ui-invalid.wasm",
        "ui-overflow.wasm",
        "ui-strings.wasm",
    ];
    assert_eq!(embedded, expected);
    for bundled in widgets::BUNDLED {
        let name = bundled.file.trim_end_matches(".wasm");
        assert_eq!(
            bundled.bytes,
            package(name),
            "{} is the built package",
            bundled.file
        );
    }
    // The releases the probe's source serves: none outside the probe build, whose only release
    // source is then GitHub's (feature 010, research R16)
    let releases: Vec<&str> = widgets::PROBE_RELEASES
        .iter()
        .map(|(name, _)| *name)
        .collect();
    #[cfg(not(feature = "shell-probe"))]
    assert!(releases.is_empty(), "{releases:?}");
    #[cfg(feature = "shell-probe")]
    assert_eq!(releases, vec!["install-basic", "compile-heavy"]);
}

// R2: the bundled approval, only for packages that declare nothing

#[test]
fn the_showcase_is_approved_and_draws() {
    let dir = tempfile::tempdir().unwrap();
    let widgets = start(dir.path(), &[showcase()]);
    let frames = widgets.list();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].handle, "w0");
    assert_eq!(frames[0].name, "Vitrine");
    let icon = frames[0].icon.as_ref().expect("the showcase has an icon");
    assert_eq!(icon.rgba.len(), (icon.size * icon.size * 4) as usize);
    assert!(content_text(&widgets.render("w0")).contains("Contagem: 0"));
    let acted = widgets.act("w0", "increment", &values(&[]));
    assert!(content_text(&acted).contains("Contagem: 1"));
    // Nothing was written for the approval: it lives in memory
    assert!(!dir.path().join("host").join("grants.json").exists());
}

/// A bundled package declaring `declared` shows the not-loaded state, and its log says why;
/// the showcase beside it still draws.
fn refused_at_start(name: &str, declared: &str) {
    let dir = tempfile::tempdir().unwrap();
    let widgets = start(dir.path(), &[showcase(), (name, package(name))]);
    let frames = widgets.list();
    assert_eq!(frames.len(), 2, "{name} keeps its frame");
    match widgets.render("w1") {
        Rendered::Error(failure) => assert_eq!(failure.kind, "not-loaded", "{name}"),
        other => panic!("{name}: expected not-loaded, got {other:?}"),
    }
    let log = log_text(&widgets, "w1");
    assert!(
        log.contains(&format!("declares {declared}")),
        "{name}: {log}"
    );
    assert!(matches!(
        widgets.act("w1", "anything", &values(&[])),
        Rendered::Refused("undeclared-action")
    ));
    assert!(content_text(&widgets.render("w0")).contains("Vitrine dos componentes"));
}

#[test]
fn a_bundled_package_declaring_a_permission_is_refused() {
    refused_at_start("hello-widget", "a permission");
}

#[test]
fn a_bundled_package_declaring_a_connector_is_refused() {
    refused_at_start("github-releases", "a connector");
}

#[test]
fn a_bundled_package_declaring_storage_is_refused() {
    refused_at_start("update-counter", "storage");
}

// FR-019: the commands' refusals

const BAD_HANDLES: &[&str] = &[
    "w1",
    "w01",
    "W0",
    "",
    " w0",
    "w0 ",
    "w-0",
    "../w0",
    "w0/../w0",
    "w99999999999999999999999",
    "showcase",
    "local/showcase",
];

#[test]
fn render_refuses_a_handle_the_host_is_not_showing() {
    let dir = tempfile::tempdir().unwrap();
    let widgets = start(dir.path(), &[showcase()]);
    for handle in BAD_HANDLES {
        assert_eq!(
            refused(&widgets.render(handle)),
            Some("unknown-widget"),
            "{handle:?}"
        );
    }
}

/// Each refusal comes before the next check, and none calls the plugin: after them all, the
/// showcase's first action still starts from no data.
#[test]
fn act_refuses_in_order_without_calling_the_plugin() {
    let dir = tempfile::tempdir().unwrap();
    let widgets = start(dir.path(), &[showcase()]);
    let long = "x".repeat(501);
    let too_many: Vec<(String, String)> = (0..21).map(|i| (format!("f{i}"), "x".into())).collect();
    let too_many = Value::Object(
        too_many
            .into_iter()
            .map(|(id, text)| (id, json!(text)))
            .collect(),
    );

    // 1. The handle, before anything else
    for handle in BAD_HANDLES {
        let acted = widgets.act(handle, "not-declared", &json!([1]));
        assert_eq!(refused(&acted), Some("unknown-widget"), "{handle:?}");
    }
    // 3. The action, before the values
    for action in [
        "not-declared",
        "",
        "ADD",
        "add ",
        "plugin:opener|open_url",
        "increment\0",
    ] {
        let acted = widgets.act("w0", action, &json!([1]));
        assert_eq!(refused(&acted), Some("undeclared-action"), "{action:?}");
    }
    // 4. The values, before the secret check
    for bad in [
        json!([]),
        json!("title"),
        json!(null),
        json!({ "title": 1 }),
        json!({ "title": true }),
        json!({ "title": null }),
        json!({ "title": ["x"] }),
        json!({ "title": { "x": "y" } }),
        json!({ "title": long }),
        too_many,
    ] {
        let acted = widgets.act("w0", "add", &bad);
        assert_eq!(refused(&acted), Some("bad-values"), "{bad}");
    }
    // Twenty values of 500 characters are within the bounds
    let full = Value::Object(
        (0..20)
            .map(|i| (format!("f{i}"), json!("y".repeat(500))))
            .collect(),
    );
    assert_eq!(refused(&widgets.act("w0", "increment", &full)), None);
    // That action was the first to reach the plugin
    let text = content_text(&widgets.render("w0"));
    assert!(text.contains("Contagem: 1"), "{text}");
    assert!(
        !text.contains("xxxxxxxx"),
        "no refused value reached the plugin"
    );
}

/// A widget whose action is running refuses another render or action, before the action name
/// is checked; another widget still answers.
#[test]
fn a_busy_widget_refuses_render_and_act() {
    let dir = tempfile::tempdir().unwrap();
    // The time limit ends the loop, not the CPU budget, which a debug build spends quickly
    let limits = Limits {
        update_time_limit: Duration::from_secs(2),
        fuel: u64::MAX / 2,
        ..Limits::default()
    };
    let probe = packed(
        "action-probe",
        &json!({
            "manifest_version": 1,
            "id": "local/action-probe",
            "version": "0.1.0",
            "author": "Dashboard contributors",
            "permissions": [],
            "actions": [{ "name": "loop" }]
        }),
    );
    let widgets = std::sync::Arc::new(start_on(
        app_runtime(dir.path(), limits),
        &[("action-probe.wasm", probe), showcase()],
    ));
    let running = {
        let widgets = std::sync::Arc::clone(&widgets);
        std::thread::spawn(move || widgets.act("w0", "loop", &values(&[])))
    };
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(refused(&widgets.render("w0")), Some("widget-busy"));
    assert_eq!(
        refused(&widgets.act("w0", "not-declared", &json!([1]))),
        Some("widget-busy")
    );
    assert!(content_text(&widgets.render("w1")).contains("Vitrine"));
    match running.join().unwrap() {
        Rendered::Error(failure) => assert_eq!(failure.kind, "action-failed"),
        other => panic!("the loop ends as action-failed, got {other:?}"),
    }
    assert_eq!(refused(&widgets.render("w0")), None, "no longer busy");
}

/// A key the app's vault holds, connected to the test catalog's `catalog/keyed`.
fn runtime_holding(dir: &Path, key: &str) -> Runtime {
    let connector = json!({
        "connector_version": 1,
        "id": "catalog/keyed",
        "version": "1.0.0",
        "author": "Tests",
        "description": "A service with a typed key",
        "reserves": ["keyed.test"],
        "destinations": { "api": "https://api.keyed.test" },
        "credential": {
            "kind": "key", "required": true, "destinations": ["api"],
            "header": "Authorization", "scheme": "Bearer"
        },
        "operations": {
            "look": { "kind": "read", "method": "GET", "destination": "api", "path": "/look",
                      "response": { "format": "json" } }
        }
    });
    let runtime = Runtime::with_test_network(
        Limits::default(),
        widgets::app_options(dir),
        runtime::TestNetwork {
            catalog: vec![connector.to_string()],
            ..runtime::TestNetwork::default()
        },
    )
    .expect("the runtime starts");
    runtime.connect_key("catalog/keyed", key).unwrap();
    runtime
}

/// FR-025: a value holding a secret the host holds is refused before the plugin runs, and the
/// log names the field, never the value.
#[test]
fn a_form_value_holding_a_held_secret_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let key = "k3y/held+by=the-host";
    let widgets = start_on(runtime_holding(dir.path(), key), &[showcase()]);
    // As typed, inside other text, and in two of the encodings the network layer withholds
    for title in [
        key.to_owned(),
        format!("see {key}"),
        key.replace('/', "\\/"),
        key.replace('/', "%2F")
            .replace('+', "%2B")
            .replace('=', "%3D"),
    ] {
        let acted = widgets.act("w0", "add", &values(&[("title", &title)]));
        assert_eq!(refused(&acted), Some("secret-in-form"), "{title}");
    }
    let log = log_text(&widgets, "w0");
    assert!(log.contains("title"), "{log}");
    assert!(!log.contains("k3y"), "the log never holds the value: {log}");
    // An innocent value passes, and is the first the plugin sees
    let acted = widgets.act("w0", "add", &values(&[("title", "Revisar PR")]));
    let text = content_text(&acted);
    assert!(
        text.contains("Revisar PR") && !text.contains("k3y"),
        "{text}"
    );
}

/// A keychain that cannot be read refuses the form: the check fails closed.
/// A held secret typed in pieces across a form's fields is caught read in the order the form
/// draws them, which is not the order of their ids (title, priority, estimate, done).
#[test]
fn a_held_secret_split_across_fields_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let key = "k3y/held+by=the-host";
    let widgets = start_on(runtime_holding(dir.path(), key), &[showcase()]);
    assert!(content_text(&widgets.render("w0")).contains("Vitrine dos componentes"));
    let acted = widgets.act(
        "w0",
        "add",
        &values(&[
            ("title", "k3y/held+"),
            ("priority", ""),
            ("estimate", "by=the-host"),
            ("done", "false"),
        ]),
    );
    assert_eq!(refused(&acted), Some("secret-in-form"));
    let log = log_text(&widgets, "w0");
    assert!(log.contains("the fields read together"), "{log}");
    assert!(!log.contains("k3y"), "the log never holds the value: {log}");
    // Pieces that make no secret together pass
    let acted = widgets.act(
        "w0",
        "add",
        &values(&[("title", "Revisar"), ("estimate", "2"), ("done", "false")]),
    );
    assert!(content_text(&acted).contains("Revisar"));
}

#[test]
fn an_unreadable_keychain_refuses_the_form() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = runtime_holding(dir.path(), "k3y-unreadable");
    let origin = runtime.list_accounts().unwrap()[0].origin.clone();
    runtime.test_vault_fail_next(
        "catalog/keyed",
        &origin,
        runtime::VaultFailure::NoStorageAccess,
    );
    let widgets = start_on(runtime, &[showcase()]);
    let acted = widgets.act("w0", "add", &values(&[("title", "anything")]));
    assert_eq!(refused(&acted), Some("secret-in-form"));
}

// HS-02: the drawn name

#[test]
fn the_drawn_name_is_plain_text_within_its_bounds() {
    let showcase = "local/showcase";
    // Markup stays text: the page draws it as text
    assert_eq!(
        drawn_name(Some("<b>Vitrine</b>"), showcase),
        "<b>Vitrine</b>"
    );
    // Bidirectional overrides and isolates, and other format characters, are removed
    assert_eq!(drawn_name(Some("Vit\u{202E}enir"), showcase), "Vitenir");
    assert_eq!(
        drawn_name(Some("\u{2066}Vitrine\u{2069}"), showcase),
        "Vitrine"
    );
    assert_eq!(
        drawn_name(Some("Vi\u{200B}tri\u{200D}ne\u{FEFF}\u{00AD}"), showcase),
        "Vitrine"
    );
    assert_eq!(
        drawn_name(Some("Vitrine\u{E0041}\u{E007F}"), showcase),
        "Vitrine"
    );
    // Whitespace collapsed and trimmed
    assert_eq!(
        drawn_name(Some("  Minha \t  vitrine "), showcase),
        "Minha vitrine"
    );
    // The longest accepted name is cut at 40 characters, with an ellipsis
    let long = "é".repeat(60);
    let drawn = drawn_name(Some(&long), showcase);
    assert_eq!(drawn.chars().count(), 40);
    assert!(drawn.ends_with('…'));
    assert_eq!(drawn_name(Some(&"a".repeat(40)), showcase), "a".repeat(40));
    // Characters that draw nothing are removed too
    assert_eq!(
        drawn_name(Some("Vi\u{034F}trine\u{FE0F}\u{E0100}"), showcase),
        "Vitrine"
    );
    // Nothing left, or no name: the id's last part
    assert_eq!(drawn_name(Some("\u{202E}\u{200B}"), showcase), "showcase");
    assert_eq!(
        drawn_name(Some("\u{3164}\u{115F}\u{1160}\u{FFA0}\u{2800}"), showcase),
        "showcase"
    );
    assert_eq!(drawn_name(Some("   "), showcase), "showcase");
    assert_eq!(drawn_name(None, showcase), "showcase");
}
