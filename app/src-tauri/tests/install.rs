//! The app's installation requests, below the page (specs/010-plugin-installation/
//! `contracts/window-commands.md`; tasks T019, T020): installed widgets load at start after the
//! cleanup of what an interrupted installation left, one request at a time, a token that names
//! only the request the host opened, choices within their bounds, an approval of exactly what was
//! shown, and the safe choice whenever the person cancels, the window closes or the app quits.
//! The compile process is this crate's own binary, as in the app.
//!
//! Part of the security core (constitution v2.7.0, Principle IX).

mod common;

use common::{App, BASIC, approved_handle, package_named, refused, request};
use dashboard_lib::install::{Approved, Judged};
use runtime::install::Installed;
use runtime::manifest::PluginId;
use runtime::package;
use serde_json::{Value, json};

#[test]
fn a_widget_installed_through_the_requests_shows_its_owner_and_loads_again_at_start() {
    let app = App::new();
    let widgets = app.start();
    let (token, choice) = request(&widgets.install_start(BASIC));
    let handle = approved_handle(&widgets.request_approve(&token, &choice, &json!([])));
    let frame = widgets
        .list()
        .into_iter()
        .find(|frame| frame.handle == handle)
        .expect("listed");
    assert_eq!(
        frame.source.as_deref(),
        Some("github.com/dashboard-fixtures")
    );
    assert_eq!(frame.name, "Instalação básica");
    drop(widgets);

    let again = app.start();
    let frames = again.list();
    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0].source.as_deref(),
        Some("github.com/dashboard-fixtures")
    );
    // Installing it a second time is refused: it is installed
    assert_eq!(
        refused(&again.install_start(BASIC)),
        Some("already-installed")
    );
}

#[test]
fn the_start_up_cleanup_removes_what_no_record_names() {
    let app = App::new();
    let layout = app.layout();
    // A package directory nobody names, an entry with no grant, a repository grant with no entry
    let packages = app.host().join("packages").join("feed");
    std::fs::create_dir_all(&packages).unwrap();
    let package = package_named("install-basic");
    let digest = package::digest(&package);
    layout
        .installed()
        .insert(
            "github.com/dashboard-fixtures/orphan",
            Installed {
                repository_id: 9,
                tag: "v1.0.0".to_owned(),
                version: "1.0.0".to_owned(),
                package_digest: digest.clone(),
                compiled_digest: digest,
                installed_at: "2026-11-12T20:00:00Z".to_owned(),
            },
        )
        .unwrap();
    let grants_path = app.host().join("grants.json");
    std::fs::write(
        &grants_path,
        json!({
            "record_version": 5,
            "plugins": { "github.com/dashboard-fixtures/stray": {
                "origin": "github:3", "granted": [], "approved_version": "1.0.0",
                "approved_digest": "sha256:00", "connectors": {}
            } }
        })
        .to_string(),
    )
    .unwrap();
    let widgets = app.start();
    assert!(widgets.list().is_empty());
    app.assert_nothing_kept();
}

#[test]
fn an_unreadable_record_removes_nothing() {
    let app = App::new();
    let widgets = app.start();
    let (token, choice) = request(&widgets.install_start(BASIC));
    approved_handle(&widgets.request_approve(&token, &choice, &json!([])));
    drop(widgets);
    let grants_path = app.host().join("grants.json");
    let kept = std::fs::read(&grants_path).unwrap();
    std::fs::write(&grants_path, b"damaged").unwrap();
    let _widgets = app.start();
    assert_eq!(app.layout().installed().read().unwrap().len(), 1);
    let installed = app.layout().installed().get(BASIC).unwrap().unwrap();
    assert!(
        app.layout().read(&installed).is_ok(),
        "its files were removed"
    );
    std::fs::write(&grants_path, kept).unwrap();
}

/// `one-request`: a second request while one waits is refused, and the first still stands.
#[test]
fn one_request_at_a_time() {
    let app = App::new();
    app.source.publish(
        "dashboard-fixtures/install-more",
        8,
        "v0.2.0",
        package_named("install-more"),
    );
    let widgets = app.start();
    let (token, choice) = request(&widgets.install_start(BASIC));
    assert_eq!(
        refused(&widgets.install_start("github.com/dashboard-fixtures/compile-heavy")),
        Some("request-pending")
    );
    approved_handle(&widgets.request_approve(&token, &choice, &json!([])));
}

/// `token-replayed`: a spent token, or one the host never gave, names nothing.
#[test]
fn a_token_names_only_the_request_the_host_opened() {
    let app = App::new();
    let widgets = app.start();
    let (token, choice) = request(&widgets.install_start(BASIC));
    assert!(matches!(
        widgets.request_approve("0123456789abcdef0123456789abcdef", &choice, &json!([])),
        Approved::Refused {
            refused: "unknown-request"
        }
    ));
    approved_handle(&widgets.request_approve(&token, &choice, &json!([])));
    assert!(matches!(
        widgets.request_approve(&token, &choice, &json!([])),
        Approved::Refused {
            refused: "unknown-request"
        }
    ));
    assert!(matches!(
        widgets.request_judge(&token, &json!([])),
        Judged::Refused {
            refused: "unknown-request"
        }
    ));
}

/// `optional-bounds`: only a list of the package's own optionals, each once, at most 48.
#[test]
fn choices_are_only_the_packages_own_optionals() {
    let app = App::new();
    let widgets = app.start();
    let (token, _) = request(&widgets.install_start(BASIC));
    let too_many: Vec<Value> = (0..49).map(|index| json!(format!("x{index}"))).collect();
    for bad in [
        json!("catalog/github"),
        json!({ "catalog/github": true }),
        json!(["catalog/elsewhere"]),
        json!(["storage"]),
        json!(["log"]),
        json!(["catalog/github", "catalog/github"]),
        json!([1]),
        Value::Array(too_many),
    ] {
        assert!(
            matches!(
                widgets.request_judge(&token, &bad),
                Judged::Refused {
                    refused: "bad-choice"
                }
            ),
            "{bad}"
        );
        assert!(
            matches!(
                widgets.request_approve(&token, "sha256:00", &bad),
                Approved::Refused {
                    refused: "bad-choice"
                }
            ),
            "{bad}"
        );
    }
    assert!(matches!(
        widgets.request_judge(&token, &json!(["catalog/github"])),
        Judged::Judged { .. }
    ));
}

#[test]
fn an_address_that_is_not_a_repository_is_refused() {
    let app = App::new();
    let widgets = app.start();
    for address in [
        "file:///tmp/x.wasm",
        "/tmp/x.wasm",
        "https://example.com/x",
        "gitlab.com/a/b",
        "github.com/only-owner",
    ] {
        assert_eq!(
            refused(&widgets.install_start(address)),
            Some("bad-address"),
            "{address}"
        );
    }
    app.assert_nothing_kept();
}

/// `approval-changed`: an approval with a choice digest that is not the one the host returned.
#[test]
fn an_approval_of_another_choice_grants_nothing() {
    let app = App::new();
    let widgets = app.start();
    let (token, _) = request(&widgets.install_start(BASIC));
    let error = match widgets.request_approve(&token, "sha256:00", &json!([])) {
        Approved::Error { error } => error,
        other => panic!("expected an error, got {other:?}"),
    };
    assert_eq!(error.kind, "approval-changed");
    app.assert_nothing_kept();
}

/// `cancel-keeps-nothing`
#[test]
fn a_cancel_keeps_nothing() {
    let app = App::new();
    let widgets = app.start();
    let (token, choice) = request(&widgets.install_start(BASIC));
    widgets.request_cancel(&token);
    assert!(matches!(
        widgets.request_approve(&token, &choice, &json!([])),
        Approved::Refused {
            refused: "unknown-request"
        }
    ));
    app.assert_nothing_kept();
    // A cancel with a token the host never gave is ignored
    widgets.request_cancel("nothing");
}

/// `window-closed-mid-dialog`: the window closing takes the safe choice.
#[test]
fn the_window_closing_takes_the_safe_choice() {
    let app = App::new();
    let widgets = app.start();
    let (token, choice) = request(&widgets.install_start(BASIC));
    widgets.stop_requests();
    assert!(matches!(
        widgets.request_approve(&token, &choice, &json!([])),
        Approved::Refused {
            refused: "unknown-request"
        }
    ));
    app.assert_nothing_kept();
}

#[test]
fn a_bundled_widget_is_not_an_installed_one() {
    let app = App::new();
    let widgets = app.start();
    assert_eq!(refused(&widgets.update_start("w0")), Some("unknown-widget"));
    let _ = PluginId::parse(BASIC).unwrap();
}
