//! Helpers of the app's installation tests: an in-memory release source and the app's widget
//! host on a fresh data directory, with this crate's own binary as the compile process.

#![allow(dead_code, reason = "each test file uses some of them")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dashboard_lib::install::{Approved, Started};
use dashboard_lib::widgets::{self, Installs, Widgets};
use runtime::compile::{CompileLimits, Compiler};
use runtime::install::{Identity, Layout, Release, ReleaseSource, Repository};
use runtime::{ErrorKind, Limits, Runtime, RuntimeError, package};

pub fn package_named(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/plugins")
        .join(format!("{name}.wasm"));
    std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}; run `cargo xtask plugins` first",
            path.display()
        )
    })
}

/// A repository: its permanent identifier, its latest release's tag and package.
type Repo = (u64, String, Vec<u8>);

/// Releases in memory: by name, a permanent identifier; by identifier, a tag and a package.
#[derive(Default)]
pub struct Source {
    repos: Mutex<BTreeMap<String, Repo>>,
}

impl Source {
    pub fn publish(&self, full_name: &str, id: u64, tag: &str, package: Vec<u8>) {
        self.repos
            .lock()
            .unwrap()
            .insert(full_name.to_owned(), (id, tag.to_owned(), package));
    }

    fn by_id(&self, id: u64) -> Option<(String, String, Vec<u8>)> {
        self.repos
            .lock()
            .unwrap()
            .iter()
            .find(|(_, (each, _, _))| *each == id)
            .map(|(name, (_, tag, package))| (name.clone(), tag.clone(), package.clone()))
    }
}

fn identity(full_name: &str, id: u64) -> Identity {
    Identity {
        repository: Repository::from_plugin_id(&format!("github.com/{full_name}")).unwrap(),
        repository_id: id,
    }
}

/// The source as the host holds it, shared with the test.
pub struct Shared(Arc<Source>);

impl ReleaseSource for Shared {
    fn identity(&self, repository: &Repository) -> Result<Identity, RuntimeError> {
        let repos = self.0.repos.lock().unwrap();
        let (id, _, _) = repos
            .get(&repository.full_name())
            .ok_or_else(|| RuntimeError::new(ErrorKind::SourceUnreachable, "no such repository"))?;
        Ok(identity(&repository.full_name(), *id))
    }

    fn identity_by_id(&self, repository_id: u64) -> Result<Identity, RuntimeError> {
        let (name, _, _) = self
            .0
            .by_id(repository_id)
            .ok_or_else(|| RuntimeError::new(ErrorKind::RepositoryChanged, "deleted"))?;
        Ok(identity(&name, repository_id))
    }

    fn latest_release(&self, identity: &Identity) -> Result<Release, RuntimeError> {
        let (_, tag, package) = self
            .0
            .by_id(identity.repository_id)
            .ok_or_else(|| RuntimeError::new(ErrorKind::NoPackageAsset, "no release"))?;
        Ok(Release {
            version: runtime::semver::Version::parse(tag.trim_start_matches('v')).unwrap(),
            tag,
            asset_id: identity.repository_id,
            asset_name: "widget.wasm".to_owned(),
            asset_size: package.len() as u64,
            asset_digest: Some(package::digest(&package)),
            found_by_name: false,
        })
    }

    fn download(&self, identity: &Identity, _release: &Release) -> Result<Vec<u8>, RuntimeError> {
        self.0
            .by_id(identity.repository_id)
            .map(|(_, _, package)| package)
            .ok_or_else(|| RuntimeError::new(ErrorKind::NoPackageAsset, "no release"))
    }
}

/// The app's widget host on a fresh data directory, with the in-memory source.
pub struct App {
    pub dir: tempfile::TempDir,
    pub source: Arc<Source>,
}

impl App {
    pub fn new() -> App {
        let app = App {
            dir: tempfile::tempdir().unwrap(),
            source: Arc::new(Source::default()),
        };
        app.source.publish(
            "dashboard-fixtures/install-basic",
            7,
            "v0.1.0",
            package_named("install-basic"),
        );
        app
    }

    pub fn host(&self) -> PathBuf {
        self.dir.path().join("host")
    }

    pub fn layout(&self) -> Layout {
        Layout::new(self.host())
    }

    pub fn start(&self) -> Widgets {
        let runtime = Runtime::new(Limits::default(), widgets::app_options(self.dir.path()))
            .expect("the runtime starts");
        let layout = self.layout();
        let installs = Installs {
            grants: Arc::new(layout.grants()),
            layout,
            compiler: Compiler::with_executable(
                env!("CARGO_BIN_EXE_dashboard"),
                CompileLimits::default(),
            ),
            source: Box::new(Shared(Arc::clone(&self.source))),
        };
        Widgets::start_with(runtime, &[], Some(installs))
    }

    pub fn assert_nothing_kept(&self) {
        let layout = self.layout();
        assert!(layout.installed().read().unwrap().is_empty());
        assert!(layout.grants().entries().unwrap().is_empty());
        let left: Vec<_> = std::fs::read_dir(self.host().join("packages"))
            .map(|dirs| dirs.flatten().map(|dir| dir.file_name()).collect())
            .unwrap_or_default();
        assert!(left.is_empty(), "{left:?}");
    }
}

pub const BASIC: &str = "github.com/dashboard-fixtures/install-basic";

pub fn request(started: &Started) -> (String, String) {
    match started {
        Started::Request { request, model } => (request.clone(), model.choice.clone()),
        other => panic!("expected a request, got {other:?}"),
    }
}

pub fn refused(started: &Started) -> Option<&str> {
    match started {
        Started::Refused { refused } => Some(refused),
        _ => None,
    }
}

pub fn approved_handle(approved: &Approved) -> String {
    match approved {
        Approved::Approved { approved } => approved.handle.clone(),
        other => panic!("expected an approval, got {other:?}"),
    }
}
