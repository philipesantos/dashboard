//! The probe's release source (specs/010-plugin-installation/research.md, R16): the releases the
//! probe build embeds, served as `github.com/dashboard-fixtures/<name>` at the version their
//! manifest declares, so the probe installs through the real installer, compile process and dialog
//! without the network. Only in the `shell-probe` build, which runs only with `DASHBOARD_PROBE=1`.

use runtime::install::{Identity, Release, ReleaseSource, Repository};
use runtime::manifest::Manifest;
use runtime::{ErrorKind, RuntimeError, package};

/// The owner every probe release is published under.
const OWNER: &str = "dashboard-fixtures";

pub struct ProbeSource {
    /// Each release: its repository's name, its permanent identifier, its tag and its package.
    releases: Vec<(String, u64, String, &'static [u8])>,
}

impl ProbeSource {
    pub fn new(embedded: &[(&str, &'static [u8])]) -> ProbeSource {
        let releases = embedded
            .iter()
            .zip(1u64..)
            .map(|((name, bytes), id)| {
                let version = package::manifest_section(bytes)
                    .ok()
                    .and_then(|section| Manifest::parse(section).ok())
                    .map_or_else(
                        || "0.0.0".to_owned(),
                        |manifest| manifest.version().to_owned(),
                    );
                ((*name).to_owned(), id, format!("v{version}"), *bytes)
            })
            .collect();
        ProbeSource { releases }
    }

    fn find(
        &self,
        matches: impl Fn(&(String, u64, String, &'static [u8])) -> bool,
    ) -> Option<&(String, u64, String, &'static [u8])> {
        self.releases.iter().find(|release| matches(release))
    }
}

fn identity(name: &str, id: u64) -> Result<Identity, RuntimeError> {
    let repository = Repository::from_plugin_id(&format!("github.com/{OWNER}/{name}"))
        .ok_or_else(|| RuntimeError::new(ErrorKind::SourceUnreachable, "not a repository"))?;
    Ok(Identity {
        repository,
        repository_id: id,
    })
}

impl ReleaseSource for ProbeSource {
    fn identity(&self, repository: &Repository) -> Result<Identity, RuntimeError> {
        let (name, id, _, _) = self
            .find(|(name, _, _, _)| repository.owner() == OWNER && repository.name() == name)
            .ok_or_else(|| {
                RuntimeError::new(
                    ErrorKind::SourceUnreachable,
                    "the probe has no such repository",
                )
            })?;
        identity(name, *id)
    }

    fn identity_by_id(&self, repository_id: u64) -> Result<Identity, RuntimeError> {
        let (name, id, _, _) = self
            .find(|(_, id, _, _)| *id == repository_id)
            .ok_or_else(|| RuntimeError::new(ErrorKind::RepositoryChanged, "deleted"))?;
        identity(name, *id)
    }

    fn latest_release(&self, identity: &Identity) -> Result<Release, RuntimeError> {
        let (_, _, tag, bytes) = self
            .find(|(_, id, _, _)| *id == identity.repository_id)
            .ok_or_else(|| RuntimeError::new(ErrorKind::NoPackageAsset, "no release"))?;
        let version = runtime::semver::Version::parse(tag.trim_start_matches('v'))
            .map_err(|_| RuntimeError::new(ErrorKind::TagMismatch, "a bad tag"))?;
        Ok(Release {
            tag: tag.clone(),
            version,
            asset_id: identity.repository_id,
            asset_name: "widget.wasm".to_owned(),
            asset_size: bytes.len() as u64,
            asset_digest: Some(package::digest(bytes)),
            found_by_name: false,
        })
    }

    fn download(&self, identity: &Identity, _release: &Release) -> Result<Vec<u8>, RuntimeError> {
        self.find(|(_, id, _, _)| *id == identity.repository_id)
            .map(|(_, _, _, bytes)| bytes.to_vec())
            .ok_or_else(|| RuntimeError::new(ErrorKind::NoPackageAsset, "no release"))
    }
}
