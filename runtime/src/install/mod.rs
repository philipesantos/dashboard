//! Installing a plugin from its author's repository (`specs/010-plugin-installation/`): where
//! releases come from ([`github`]), the limits of fetching them, and the repository a plugin id
//! names.

pub mod dialog;
pub mod github;
pub mod record;

use std::time::Duration;

use crate::manifest::is_repository_id;
use crate::{ErrorKind, RuntimeError};

pub use dialog::{DialogKind, DialogModel, ModelInput, model};
pub use github::{GitHub, Identity, Release, ReleaseSource};
pub use record::{Installed, InstalledStore};

/// The limits of fetching a release (`data-model.md`, Release fetch limits).
#[derive(Debug, Clone)]
pub struct FetchLimits {
    /// The most of one API answer read.
    pub api_body_bytes: usize,
    /// The time one API request may take, redirects included.
    pub api_time: Duration,
    /// The time the package's download may take, redirects included.
    pub package_time: Duration,
    /// The redirects a download may follow, only to GitHub's two asset hosts.
    pub max_redirects: usize,
}

impl Default for FetchLimits {
    fn default() -> Self {
        FetchLimits {
            api_body_bytes: 256 * 1024,
            api_time: Duration::from_secs(10),
            package_time: Duration::from_secs(60),
            max_redirects: 3,
        }
    }
}

/// A repository on `github.com`, as a plugin id names it: `github.com/<owner>/<repo>`, lowercase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    owner: String,
    name: String,
}

impl Repository {
    /// The repository a plugin id names, or `None` for a `local/` id or anything else.
    pub fn from_plugin_id(id: &str) -> Option<Repository> {
        if !is_repository_id(id) {
            return None;
        }
        let path = id.strip_prefix("github.com/")?;
        let (owner, name) = path.split_once('/')?;
        Some(Repository {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// `owner/name`, as GitHub's `full_name`.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// `github.com/owner/name`, the plugin id.
    pub fn plugin_id(&self) -> String {
        format!("github.com/{}/{}", self.owner, self.name)
    }
}

fn unreachable_source(detail: impl AsRef<str>) -> RuntimeError {
    RuntimeError::new(ErrorKind::SourceUnreachable, detail)
}
