//! Installing from a repository (`specs/010-plugin-installation/`, FR-001 to FR-008, FR-013 to
//! FR-015; research R4, R5; tasks T022, T023): on an in-memory release source and the real
//! compile process, a release installs only when its manifest names the repository and its tag
//! the version, writes in order and only on an approval of what was shown, keeps nothing when it
//! is refused, and a crash between two writes leaves nothing usable after the next start.

use std::sync::Arc;

use runtime::compile::{CompileLimits, Compiler};
use runtime::install::dialog::DialogKind;
use runtime::install::{Installer, Layout};
use runtime::{ErrorKind, Limits, Runtime, RuntimeOptions, package};
use serde_json::json;
use tempfile::TempDir;

use crate::common::releases::MemorySource;
use crate::common::{fixture, repack};

/// A runtime, a compile process, a release source and the app's host directory.
struct App {
    _dir: TempDir,
    runtime: Runtime,
    compiler: Compiler,
    source: MemorySource,
    layout: Layout,
}

impl App {
    fn new() -> App {
        let dir = TempDir::new().expect("a temporary directory");
        let host = dir.path().join("host");
        let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(&host))
            .expect("the runtime starts");
        App {
            runtime,
            compiler: Compiler::with_executable(
                env!("CARGO_BIN_EXE_runtime"),
                CompileLimits::default(),
            ),
            source: MemorySource::new(),
            layout: Layout::new(&host),
            _dir: dir,
        }
    }

    fn installer(&self) -> Installer<'_> {
        Installer {
            runtime: &self.runtime,
            source: &self.source,
            compiler: &self.compiler,
            layout: &self.layout,
            cancel: None,
        }
    }

    /// Prepares and installs `owner/repo` with no optional turned on.
    fn install(&self, address: &str) -> Result<runtime::install::Installed, runtime::RuntimeError> {
        let installer = self.installer();
        let repository = Installer::parse_address(address).expect("a repository address");
        let prepared = installer.prepare_install(&repository)?;
        let choice = installer.model(&prepared, &[]).choice;
        installer.commit(&prepared, &choice, &[])
    }

    /// Nothing of an installation is on disk.
    fn assert_nothing_kept(&self) {
        assert!(self.layout.installed().read().expect("readable").is_empty());
        assert!(self.layout.grants().entries().expect("readable").is_empty());
        let packages = self._dir.path().join("host").join("packages");
        let left: Vec<_> = std::fs::read_dir(&packages)
            .map(|dirs| dirs.flatten().map(|dir| dir.file_name()).collect())
            .unwrap_or_default();
        assert!(left.is_empty(), "{left:?}");
    }
}

#[test]
fn a_release_installs_and_loads_from_its_compiled_form() {
    let app = App::new();
    let package = fixture("install-basic");
    app.source.publish(
        7,
        "dashboard-fixtures/install-basic",
        "v0.1.0",
        package.clone(),
    );
    let installed = app
        .install("github.com/dashboard-fixtures/install-basic")
        .expect("installed");
    assert_eq!(installed.repository_id, 7);
    assert_eq!(installed.tag, "v0.1.0");
    assert_eq!(installed.version, "0.1.0");
    assert_eq!(installed.package_digest, package::digest(&package));
    assert_eq!(installed.origin(), "github:7");

    let id = "github.com/dashboard-fixtures/install-basic";
    let record = app
        .layout
        .installed()
        .get(id)
        .expect("readable")
        .expect("recorded");
    assert_eq!(record, installed);
    let grants = Arc::new(app.layout.grants());
    let plugin = app
        .installer()
        .load(id, &installed, Arc::clone(&grants))
        .expect("it loads");
    assert!(plugin.render("{}").result.is_ok());
    // Nothing to clean after a complete installation
    assert_eq!(app.layout.cleanup().expect("clean"), Default::default());
}

/// `identity-mismatch`: a package whose manifest names another repository.
#[test]
fn a_package_that_names_another_repository_is_refused() {
    let app = App::new();
    app.source
        .publish(7, "someone/else", "v0.1.0", fixture("install-basic"));
    let error = app.install("github.com/someone/else").expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::IdentityMismatch, "{error}");
    app.assert_nothing_kept();
}

/// `tag-mismatch`: the tag's version is not the manifest's.
#[test]
fn a_tag_that_is_not_the_manifests_version_is_refused() {
    let app = App::new();
    app.source.publish(
        7,
        "dashboard-fixtures/install-basic",
        "v9.9.9",
        fixture("install-basic"),
    );
    let error = app
        .install("github.com/dashboard-fixtures/install-basic")
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::TagMismatch, "{error}");
    app.assert_nothing_kept();
}

/// `oversized-asset` past the source: a package over the cap never reaches the compile process.
#[test]
fn a_package_over_the_cap_is_refused_before_compiling() {
    let app = App::new();
    let too_big = vec![0; Limits::default().max_package_bytes + 1];
    app.source
        .publish(7, "dashboard-fixtures/install-basic", "v0.1.0", too_big);
    let error = app
        .install("github.com/dashboard-fixtures/install-basic")
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::InvalidPackage, "{error}");
    app.assert_nothing_kept();
}

/// `name-swapped-mid-install`: once the name has led to a permanent identifier, everything
/// after is asked by that identifier, whatever the name leads to by then.
#[test]
fn a_name_that_changes_hands_mid_install_serves_nothing_of_the_other() {
    let app = App::new();
    let ours = fixture("install-basic");
    app.source.publish(
        7,
        "dashboard-fixtures/install-basic",
        "v0.1.0",
        ours.clone(),
    );
    app.source.publish(
        8,
        "dashboard-fixtures/install-basic",
        "v0.1.0",
        fixture("install-more"),
    );
    app.source
        .name_leads_to("dashboard-fixtures/install-basic", &[7, 8]);
    let installed = app
        .install("github.com/dashboard-fixtures/install-basic")
        .expect("installed");
    assert_eq!(installed.repository_id, 7);
    assert_eq!(installed.package_digest, package::digest(&ours));
    let log = app.source.log();
    assert_eq!(
        log.iter()
            .filter(|line| line.starts_with("identity "))
            .count(),
        1,
        "{log:?}"
    );
}

/// `repository-connector`, `local-connector`: refused even when optional, naming why.
#[test]
fn a_widget_with_a_repository_or_local_connector_is_not_installed() {
    for (name, kind) in [
        ("install-repo-connector", ErrorKind::DestinationNotVerified),
        ("install-local-connector", ErrorKind::DestinationNotVerified),
    ] {
        let app = App::new();
        app.source.publish(
            7,
            &format!("dashboard-fixtures/{name}"),
            "v0.1.0",
            fixture(name),
        );
        let error = app
            .install(&format!("github.com/dashboard-fixtures/{name}"))
            .expect_err("refused");
        assert_eq!(error.kind(), kind, "{name}: {error}");
        if name == "install-repo-connector" {
            assert!(error.to_string().contains("not installed"), "{error}");
        }
        app.assert_nothing_kept();
    }
}

/// `connector-major-unavailable`: a catalog major the host does not ship, before any dialog.
#[test]
fn a_connector_major_the_host_lacks_is_refused_before_the_dialog() {
    let app = App::new();
    let manifest = json!({
        "manifest_version": 1,
        "id": "github.com/dashboard-fixtures/install-basic",
        "version": "0.1.0",
        "author": "Dashboard contributors",
        "permissions": [
            { "name": "log", "required": true },
            { "name": "storage", "required": true, "quota_bytes": 65536 }
        ],
        "connectors": [
            { "id": "catalog/github", "major": 2, "operations": ["list-releases"], "required": false }
        ],
        "actions": [{ "name": "save" }]
    });
    app.source.publish(
        7,
        "dashboard-fixtures/install-basic",
        "v0.1.0",
        repack(&fixture("action-steps"), &manifest.to_string()),
    );
    let error = app
        .install("github.com/dashboard-fixtures/install-basic")
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::ConnectorUnavailable, "{error}");
    app.assert_nothing_kept();
}

#[test]
fn an_approval_of_something_else_than_what_was_shown_writes_nothing() {
    let app = App::new();
    app.source.publish(
        7,
        "dashboard-fixtures/install-basic",
        "v0.1.0",
        fixture("install-basic"),
    );
    let installer = app.installer();
    let repository =
        Installer::parse_address("github.com/dashboard-fixtures/install-basic").unwrap();
    let prepared = installer.prepare_install(&repository).expect("prepared");
    assert_eq!(prepared.kind, DialogKind::Install);
    let shown_off = installer.model(&prepared, &[]).choice;
    // Approving with the optional on, but the choice shown with it off
    let error = installer
        .commit(&prepared, &shown_off, &["catalog/github".to_owned()])
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::ApprovalChanged);
    app.assert_nothing_kept();
}

/// `crash-mid-install`: a kill after each write of research R5 leaves nothing usable once the
/// next start has cleaned up.
#[test]
fn a_crash_between_two_writes_leaves_nothing_usable() {
    let app = App::new();
    let package = fixture("install-basic");
    app.source.publish(
        7,
        "dashboard-fixtures/install-basic",
        "v0.1.0",
        package.clone(),
    );
    let installer = app.installer();
    let repository =
        Installer::parse_address("github.com/dashboard-fixtures/install-basic").unwrap();

    // Killed after the files were written
    let prepared = installer.prepare_install(&repository).expect("prepared");
    let compiled = app.compiler.compile(&package).expect("compiled");
    app.layout.write(&package, &compiled).expect("written");
    let cleaned = app.layout.cleanup().expect("clean");
    assert_eq!(cleaned.packages.len(), 1);
    app.assert_nothing_kept();

    // Killed after the files and the grant
    app.layout.write(&package, &compiled).expect("written");
    let digest = installer.model(&prepared, &[]).digest;
    prepared
        .checked
        .install(&app.layout.grants(), "github:7", &digest, &[])
        .expect("granted");
    let cleaned = app.layout.cleanup().expect("clean");
    assert_eq!(
        cleaned.grants,
        vec!["github.com/dashboard-fixtures/install-basic".to_owned()]
    );
    app.assert_nothing_kept();

    // An installed entry whose grant is gone is removed too
    let installed = app
        .install("github.com/dashboard-fixtures/install-basic")
        .expect("installed");
    let id =
        runtime::manifest::PluginId::parse("github.com/dashboard-fixtures/install-basic").unwrap();
    app.layout.grants().remove(&id).expect("removed");
    let cleaned = app.layout.cleanup().expect("clean");
    assert_eq!(cleaned.installed, vec![id.to_string()]);
    assert_eq!(cleaned.packages, vec![installed.directory()]);
    app.assert_nothing_kept();
}

#[test]
fn an_unreadable_record_removes_nothing() {
    let app = App::new();
    app.source.publish(
        7,
        "dashboard-fixtures/install-basic",
        "v0.1.0",
        fixture("install-basic"),
    );
    let installed = app
        .install("github.com/dashboard-fixtures/install-basic")
        .expect("installed");
    let grants_path = app._dir.path().join("host").join("grants.json");
    let kept = std::fs::read(&grants_path).expect("the grant record");
    std::fs::write(&grants_path, b"not a record").expect("damaged");
    assert!(app.layout.cleanup().is_err());
    assert_eq!(app.layout.installed().read().expect("readable").len(), 1);
    std::fs::write(&grants_path, kept).expect("restored");
    assert!(
        app.layout.read(&installed).is_ok(),
        "the package was removed"
    );
}

/// A compiled form that changed is compiled again once, from the package; a package that
/// changed is refused.
#[test]
fn a_changed_compiled_form_is_compiled_again_and_a_changed_package_refused() {
    let app = App::new();
    app.source.publish(
        7,
        "dashboard-fixtures/install-basic",
        "v0.1.0",
        fixture("install-basic"),
    );
    let installed = app
        .install("github.com/dashboard-fixtures/install-basic")
        .expect("installed");
    let id = "github.com/dashboard-fixtures/install-basic";
    let dir = app
        ._dir
        .path()
        .join("host")
        .join("packages")
        .join(installed.directory());
    let compiled_path = dir.join("compiled.cwasm");
    let mut compiled = std::fs::read(&compiled_path).unwrap();
    let middle = compiled.len() / 2;
    compiled[middle] ^= 1;
    std::fs::write(&compiled_path, compiled).unwrap();
    let grants = Arc::new(app.layout.grants());
    app.installer()
        .load(id, &installed, Arc::clone(&grants))
        .expect("loaded after compiling again");
    // The altered form was replaced by a fresh one, which matches its record again
    let recorded = app.layout.installed().get(id).unwrap().unwrap();
    assert_eq!(
        package::digest(&std::fs::read(&compiled_path).unwrap()),
        recorded.compiled_digest
    );

    let package_path = dir.join("package.wasm");
    let mut package = std::fs::read(&package_path).unwrap();
    let last = package.len() - 1;
    package[last] ^= 1;
    std::fs::write(&package_path, package).unwrap();
    let error = app
        .installer()
        .load(id, &recorded, grants)
        .err()
        .expect("refused");
    assert_eq!(error.kind(), ErrorKind::PackageChanged);
}
