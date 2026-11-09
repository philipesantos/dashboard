//! What the host's test targets share: a record of approvals made as the development tool makes
//! them, over the packages `cargo xtask plugins` built, and a dashboard window on GPUI's test
//! platform.
#![allow(dead_code)]

use design_tokens::Appearance;
use gpui::{Entity, TestAppContext, VisualTestContext};
use host::dashboard::Dashboard;
use host::plugins::start_runtime;
use runtime::grants::GrantStore;
use runtime::{Limits, Runtime, RuntimeOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

/// A package `cargo xtask plugins` built.
pub fn fixture(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/plugins")
        .join(format!("{name}.wasm"));
    assert!(
        path.exists(),
        "{}: run cargo xtask plugins first",
        path.display()
    );
    path.canonicalize().expect("the fixture's path")
}

/// Approves `names` in a fresh record, as the development tool does.
pub struct World {
    pub dir: TempDir,
    pub grants: Arc<GrantStore>,
    approver: Runtime,
}

impl World {
    pub fn new() -> World {
        let dir = TempDir::new().expect("a temporary directory");
        let grants = Arc::new(GrantStore::open(dir.path().join("grants.json")));
        let approver =
            Runtime::new(Limits::default(), RuntimeOptions::new(dir.path())).expect("a runtime");
        World {
            dir,
            grants,
            approver,
        }
    }

    pub fn approve(&self, name: &str) -> String {
        let path = fixture(name);
        let origin = path.to_string_lossy().into_owned();
        let bytes = std::fs::read(&path).expect("the package");
        self.approver
            .check(&bytes)
            .expect("a valid package")
            .approve(&self.grants, &origin, None)
            .expect("approved");
        origin
    }

    pub fn open<'a>(
        &self,
        cx: &'a mut TestAppContext,
    ) -> (Entity<Dashboard>, &'a mut VisualTestContext) {
        cx.update(|cx| {
            host::app::install(cx).expect("installs");
            host::vendor::gpui_base::init(cx);
        });
        let runtime = Arc::new(start_runtime(self.dir.path()).expect("a runtime"));
        let grants = Arc::clone(&self.grants);
        let (view, cx) = cx.add_window_view(move |window, cx| {
            Dashboard::new(Some(runtime), grants, Appearance::DEFAULT, window, cx)
        });
        cx.run_until_parked();
        (view, cx)
    }
}
