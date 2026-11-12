//! A release source in memory, for the installer's tests (`specs/010-plugin-installation/
//! research.md`, R16): repositories by name and by permanent identifier, each with its latest
//! release and package, and hooks that change what a name leads to between two requests.

use std::collections::BTreeMap;
use std::sync::Mutex;

use runtime::install::{Identity, Release, ReleaseSource, Repository};
use runtime::{ErrorKind, RuntimeError, package};

/// One repository the source knows.
#[derive(Clone)]
pub struct Repo {
    pub full_name: String,
    pub release: Option<(String, Vec<u8>)>,
}

#[derive(Default)]
struct State {
    /// Name → permanent identifier, as GitHub would answer now; each lookup by name takes the
    /// next answer, the last one repeating.
    names: BTreeMap<String, Vec<u64>>,
    repos: BTreeMap<u64, Repo>,
    /// Requests, in order: `identity owner/repo`, `identity-by-id 1`, `release 1`, `download 1`.
    log: Vec<String>,
}

#[derive(Default)]
pub struct MemorySource {
    state: Mutex<State>,
}

#[allow(
    dead_code,
    reason = "rename and delete serve the update cases, task T033"
)]
impl MemorySource {
    pub fn new() -> MemorySource {
        MemorySource::default()
    }

    /// The repository `full_name` (`owner/repo`) with the permanent identifier `id`, whose
    /// latest release is `tag` with `package`.
    pub fn publish(&self, id: u64, full_name: &str, tag: &str, package: Vec<u8>) {
        let mut state = self.state.lock().unwrap();
        state.names.insert(full_name.to_owned(), vec![id]);
        state.repos.insert(
            id,
            Repo {
                full_name: full_name.to_owned(),
                release: Some((tag.to_owned(), package)),
            },
        );
    }

    /// From now on, looking `full_name` up answers each of `ids` in turn, the last repeating.
    pub fn name_leads_to(&self, full_name: &str, ids: &[u64]) {
        self.state
            .lock()
            .unwrap()
            .names
            .insert(full_name.to_owned(), ids.to_vec());
    }

    /// Renames the repository `id`.
    pub fn rename(&self, id: u64, full_name: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(repo) = state.repos.get_mut(&id) {
            repo.full_name = full_name.to_owned();
        }
    }

    /// Deletes the repository `id`; its name leads nowhere.
    pub fn delete(&self, id: u64) {
        let mut state = self.state.lock().unwrap();
        if let Some(repo) = state.repos.remove(&id) {
            state.names.remove(&repo.full_name);
        }
    }

    pub fn log(&self) -> Vec<String> {
        self.state.lock().unwrap().log.clone()
    }
}

fn identity_of(id: u64, full_name: &str) -> Identity {
    Identity {
        repository: Repository::from_plugin_id(&format!("github.com/{full_name}"))
            .expect("a repository id"),
        repository_id: id,
    }
}

impl ReleaseSource for MemorySource {
    fn identity(&self, repository: &Repository) -> Result<Identity, RuntimeError> {
        let mut state = self.state.lock().unwrap();
        let name = repository.full_name();
        state.log.push(format!("identity {name}"));
        let id = match state.names.get_mut(&name) {
            Some(ids) if ids.len() > 1 => ids.remove(0),
            Some(ids) => ids[0],
            None => {
                return Err(RuntimeError::new(
                    ErrorKind::SourceUnreachable,
                    "no such repository",
                ));
            }
        };
        let repo = state
            .repos
            .get(&id)
            .ok_or_else(|| RuntimeError::new(ErrorKind::SourceUnreachable, "no such repository"))?;
        Ok(identity_of(id, &repo.full_name))
    }

    fn identity_by_id(&self, repository_id: u64) -> Result<Identity, RuntimeError> {
        let mut state = self.state.lock().unwrap();
        state.log.push(format!("identity-by-id {repository_id}"));
        let repo = state
            .repos
            .get(&repository_id)
            .ok_or_else(|| RuntimeError::new(ErrorKind::RepositoryChanged, "deleted"))?;
        Ok(identity_of(repository_id, &repo.full_name))
    }

    fn latest_release(&self, identity: &Identity) -> Result<Release, RuntimeError> {
        let mut state = self.state.lock().unwrap();
        state
            .log
            .push(format!("release {}", identity.repository_id));
        let (tag, package) = state
            .repos
            .get(&identity.repository_id)
            .and_then(|repo| repo.release.clone())
            .ok_or_else(|| RuntimeError::new(ErrorKind::NoPackageAsset, "no release"))?;
        let version = semver::Version::parse(tag.trim_start_matches('v'))
            .map_err(|_| RuntimeError::new(ErrorKind::TagMismatch, format!("bad tag {tag}")))?;
        Ok(Release {
            tag,
            version,
            asset_id: identity.repository_id,
            asset_name: "widget.wasm".to_owned(),
            asset_size: package.len() as u64,
            asset_digest: Some(package::digest(&package)),
            found_by_name: false,
        })
    }

    fn download(&self, identity: &Identity, _release: &Release) -> Result<Vec<u8>, RuntimeError> {
        let mut state = self.state.lock().unwrap();
        state
            .log
            .push(format!("download {}", identity.repository_id));
        state
            .repos
            .get(&identity.repository_id)
            .and_then(|repo| repo.release.clone())
            .map(|(_, package)| package)
            .ok_or_else(|| RuntimeError::new(ErrorKind::NoPackageAsset, "no release"))
    }
}
