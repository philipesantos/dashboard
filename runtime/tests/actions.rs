//! Actions (`specs/009-declarative-widget-ui/`, research R7 and R8): the `actor` export of
//! contract 1.4.0. A widget's buttons and forms run only actions its manifest declares; an action
//! runs like an update, its store written all or nothing, its connectors' read operations only;
//! and the data it returns reaches the next render from memory, never from the disk.

use crate::common;

use std::time::Duration;

use common::servers::{Reply, Server, TestPki};
use common::{World, fixture, repack, store_probe, stored_manifest};
use runtime::{ErrorKind, Limits, Route, Runtime, RuntimeOptions, TestNetwork};
use serde_json::{Map, Value, json};

/// A manifest for `id` with `permissions`, `actions`, and extra top-level fields.
fn manifest(id: &str, actions: &[&str], extra: Value) -> String {
    let actions: Vec<Value> = actions.iter().map(|name| json!({ "name": name })).collect();
    let mut manifest = json!({
        "manifest_version": 1,
        "id": id,
        "version": "0.1.0",
        "author": "Dashboard contributors",
        "permissions": [],
        "actions": actions
    });
    if let (Some(object), Some(extra)) = (manifest.as_object_mut(), extra.as_object()) {
        object.extend(extra.clone());
    }
    manifest.to_string()
}

/// `action-probe` (only `actor`, no capability) declaring `actions`.
fn action_probe(actions: &[&str]) -> Vec<u8> {
    repack(
        &fixture("action-probe"),
        &manifest("local/action-probe", actions, json!({})),
    )
}

/// `action-steps` with storage and the test catalog's `catalog/echo`, declaring `save`.
fn action_steps() -> Vec<u8> {
    repack(
        &fixture("action-steps"),
        &manifest(
            "local/action-steps",
            &["save"],
            json!({
                "permissions": [{ "name": "storage", "required": true, "quota_bytes": 65536 }],
                "connectors": [{ "id": "catalog/echo", "major": 1,
                                  "operations": ["look", "write"], "required": true }]
            }),
        ),
    )
}

fn values(pairs: &[(&str, &str)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), json!(value)))
        .collect()
}

fn none() -> Map<String, Value> {
    Map::new()
}

/// A configuration for `action-steps`: `steps`, then `end`.
fn steps(steps: Value, end: &str) -> String {
    json!({ "steps": steps, "end": end }).to_string()
}

/// A world with the test catalog's `catalog/echo`: `look` reads, `write` changes data, both
/// on `api.echo.test`, routed to `server`.
fn echo_world(pki: &TestPki, server: &Server) -> World {
    let connector = json!({
        "connector_version": 1,
        "id": "catalog/echo",
        "version": "1.0.0",
        "author": "Tests",
        "description": "Test services",
        "reserves": ["echo.test"],
        "destinations": { "api": "https://api.echo.test" },
        "operations": {
            "look": { "kind": "read", "method": "GET", "destination": "api", "path": "/look",
                      "response": { "format": "json" } },
            "write": { "kind": "change", "method": "POST", "destination": "api", "path": "/write",
                       "response": { "format": "json" } }
        }
    });
    World::seam(
        TestNetwork {
            routes: vec![Route {
                host: "api.echo.test".to_owned(),
                port: 443,
                answers: vec![vec![server.address()]],
            }],
            public_for_tests: vec![server.address()],
            root: pki.root(),
            catalog: vec![connector.to_string()],
        },
        false,
    )
}

fn answers(data: &Value) -> Vec<Value> {
    data["answers"]
        .as_array()
        .unwrap_or_else(|| panic!("action-steps answered {data}"))
        .clone()
}

#[test]
fn an_undeclared_action_is_refused_before_the_plugin_runs() {
    let world = World::production(false);
    let plugin = world.approve_and_load(&action_probe(&["remember"]));
    for name in ["forget", "plugin:opener|open_url", "", "REMEMBER"] {
        let report = plugin.act(name, "{}", &values(&[("note", "x")]));
        let error = report.result.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UndeclaredAction, "{name}: {error}");
        assert!(report.logs.is_empty(), "{name}: the plugin ran");
    }
    // Nothing reached the plugin, so it has no data
    assert_eq!(
        plugin.render("{}").result.unwrap()["document"]["data"],
        Value::Null
    );
}

#[test]
fn a_plugin_built_for_an_earlier_minor_has_no_actions() {
    let world = World::production(false);
    let plugin = world.approve_and_load(&store_probe(&stored_manifest(
        "local/store-probe",
        "0.1.0",
        true,
        65536,
    )));
    let error = plugin.act("save", "{}", &none()).result.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UndeclaredAction, "{error}");
}

#[test]
fn declared_actions_and_the_actor_export_go_together() {
    let world = World::production(false);
    // Actions declared by a plugin that exports no `actor`
    let without_actor = repack(
        &fixture("store-probe"),
        &manifest(
            "local/store-probe",
            &["save"],
            json!({ "permissions": [{ "name": "storage", "required": true, "quota_bytes": 65536 }] }),
        ),
    );
    let Err(error) = world.runtime.check(&without_actor) else {
        panic!("actions without actor were accepted");
    };
    assert_eq!(error.kind(), ErrorKind::InvalidManifest, "{error}");
    // `actor` exported with no action declared
    let Err(error) = world.runtime.check(&action_probe(&[])) else {
        panic!("actor without actions was accepted");
    };
    assert_eq!(error.kind(), ErrorKind::InvalidManifest, "{error}");
}

#[test]
fn an_action_receives_its_values_and_the_data_and_its_output_reaches_the_next_render() {
    let world = World::production(false);
    let plugin = world.approve_and_load(&action_probe(&["remember"]));
    let first = plugin
        .act(
            "remember",
            r#"{"lang":"pt"}"#,
            &values(&[("note", "first")]),
        )
        .result
        .unwrap();
    assert_eq!(first["remembered"], json!({ "note": "first" }));
    assert_eq!(first["seen"], Value::Null);
    let second = plugin
        .act("remember", "{}", &values(&[("note", "second")]))
        .result
        .unwrap();
    assert_eq!(
        second["seen"], first,
        "the action sees the data the last one returned"
    );

    // An actor-only plugin gets the render document with the action's data
    let document = plugin.render(r#"{"lang":"pt"}"#).result.unwrap()["document"].clone();
    assert_eq!(document["config"], json!({ "lang": "pt" }));
    assert_eq!(document["data"], second);
    assert!(document["fetched_at_ms"].as_u64().is_some(), "{document}");
    assert_eq!(document["last_update"]["outcome"], "ok");

    // A failed action keeps the previous data (none here) and says so
    let failing = world.approve_and_load(&action_probe(&["fail"]));
    assert_eq!(
        failing
            .act("fail", "{}", &none())
            .result
            .unwrap_err()
            .kind(),
        ErrorKind::PluginError
    );
    let document = failing.render("{}").result.unwrap()["document"].clone();
    assert_eq!(document["data"], Value::Null);
    assert_eq!(document["last_update"]["outcome"], "failed");
}

#[test]
fn an_action_s_data_stays_in_memory_and_never_reaches_the_disk() {
    let world = World::production(false);
    let package = action_probe(&["remember"]);
    let plugin = world.approve_and_load(&package);
    plugin
        .act(
            "remember",
            "{}",
            &values(&[("secret-ish", "typed by the person")]),
        )
        .result
        .unwrap();
    assert!(!plugin.render("{}").result.unwrap()["document"]["data"].is_null());

    // Nothing of it under the data directory
    let mut stack = vec![world.data.path().to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let text = std::fs::read(&path).unwrap_or_default();
                assert!(
                    !String::from_utf8_lossy(&text).contains("typed by the person"),
                    "{} holds the action's data",
                    path.display()
                );
            }
        }
    }

    // A new run of the host starts without it
    let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(world.data.path())).unwrap();
    let again = runtime
        .load(
            &package,
            common::TEST_ORIGIN,
            std::sync::Arc::clone(&world.grants),
        )
        .unwrap();
    assert_eq!(
        again.render("{}").result.unwrap()["document"]["data"],
        Value::Null
    );
}

#[test]
fn render_gets_the_newer_of_the_update_s_data_and_the_action_s() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    let world = echo_world(&pki, &server);
    let plugin = world.approve_and_load(&action_steps());
    let data = |plugin: &runtime::Plugin| {
        plugin.render("{}").result.unwrap()["document"]["data"]["from"].clone()
    };
    plugin.update(r#"{"n":1}"#).result.unwrap();
    assert_eq!(data(&plugin), "update");
    std::thread::sleep(Duration::from_millis(5));
    plugin
        .act("save", &steps(json!([]), "ok"), &none())
        .result
        .unwrap();
    assert_eq!(data(&plugin), "action");
    std::thread::sleep(Duration::from_millis(5));
    plugin.update(r#"{"n":2}"#).result.unwrap();
    assert_eq!(data(&plugin), "update");
}

#[test]
fn an_action_writes_the_store_all_or_nothing() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    let world = echo_world(&pki, &server);
    let plugin = world.approve_and_load(&action_steps());
    let set = |name: &str, value: &str| json!({ "op": "set", "name": name, "value": value });
    let get = |name: &str| json!({ "op": "get", "name": name });

    plugin
        .act("save", &steps(json!([set("kept", "1")]), "ok"), &none())
        .result
        .unwrap();
    for end in ["error", "trap"] {
        assert!(
            plugin
                .act("save", &steps(json!([set(end, "lost")]), end), &none())
                .result
                .is_err(),
            "{end}"
        );
    }
    let read = plugin
        .act(
            "save",
            &steps(json!([get("kept"), get("error"), get("trap")]), "ok"),
            &none(),
        )
        .result
        .unwrap();
    assert_eq!(
        answers(&read),
        [
            json!({ "ok": "1" }),
            json!({ "ok": null }),
            json!({ "ok": null })
        ]
    );
}

#[test]
fn an_action_calls_read_operations_and_never_ones_that_change_data() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    server.route("/look", Reply::json(r#"{"ok":true}"#));
    server.route("/write", Reply::json(r#"{"ok":true}"#));
    let world = echo_world(&pki, &server);
    let plugin = world.approve_and_load(&action_steps());
    let call = |operation: &str| json!({ "op": "call", "connector": "catalog/echo", "operation": operation });
    let data = plugin
        .act(
            "save",
            &steps(json!([call("look"), call("write")]), "ok"),
            &none(),
        )
        .result
        .unwrap();
    let answers = answers(&data);
    assert_eq!(answers[0], json!({ "ok": 200 }), "a read operation answers");
    let refused = answers[1]["error"].as_str().unwrap_or_default();
    assert!(refused.contains("ChangeNeedsUserAction"), "{}", answers[1]);
    let paths: Vec<String> = server.received().into_iter().map(|r| r.target).collect();
    assert_eq!(paths, ["/look"], "the change reached the service");
}

#[test]
fn an_action_runs_under_the_limits_of_an_update() {
    let world = World::production_with(
        Limits {
            update_time_limit: Duration::from_millis(500),
            ..Limits::default()
        },
        false,
    );
    let plugin = world.approve_and_load(&action_probe(&["loop", "big"]));
    let started = std::time::Instant::now();
    let error = plugin.act("loop", "{}", &none()).result.unwrap_err();
    assert!(
        matches!(
            error.kind(),
            ErrorKind::TimeLimitExceeded | ErrorKind::CpuBudgetExceeded
        ),
        "{error}"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    // Past the update's 256 KiB
    let error = plugin.act("big", "{}", &none()).result.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::OutputTooLarge, "{error}");
}
