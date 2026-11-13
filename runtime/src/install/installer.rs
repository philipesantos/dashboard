//! The installer (`specs/010-plugin-installation/research.md`, R3, R4, R5): fetches a release by
//! the repository's permanent identifier, compiles its package in the compile process, checks it
//! with that compiled form, builds the dialog's model, and, only on an approval whose choice
//! digest matches what the dialog showed, writes the package, its compiled form, the grant and
//! the installed record, in that order.
//!
//! Nothing reaches the disk before the approval: a refusal, a failure or a cancel keeps nothing.
//! At start-up [`Layout::cleanup`] removes what a crash between two writes left.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

use super::dialog::{DialogKind, DialogModel, ModelInput, model};
use super::record::{self, Installed, InstalledStore};
use super::{Identity, Release, ReleaseSource, Repository};
use crate::compile::Compiler;
use crate::grants::GrantStore;
use crate::manifest::PluginId;
use crate::{CheckedPackage, ErrorKind, Plugin, Runtime, RuntimeError, package};

/// Where the app keeps what it installed: `<app data dir>/host/`.
#[derive(Debug, Clone)]
pub struct Layout {
    host: PathBuf,
}

/// What [`Layout::cleanup`] removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cleanup {
    pub packages: Vec<String>,
    pub installed: Vec<String>,
    pub grants: Vec<String>,
}

impl Layout {
    pub fn new(host: impl Into<PathBuf>) -> Layout {
        Layout { host: host.into() }
    }

    /// The app's grant record, on disk.
    pub fn grants(&self) -> GrantStore {
        GrantStore::open(self.host.join("grants.json"))
    }

    pub fn installed(&self) -> InstalledStore {
        InstalledStore::open(self.host.join("installed.json"))
    }

    fn packages(&self) -> PathBuf {
        self.host.join("packages")
    }

    /// Takes the installations' lock, `install.lock`, exclusive, until the returned file is
    /// dropped: an installation holds it from its first write to its last, and the start-up
    /// cleanup while it runs, so a second instance of the app starting mid-installation never
    /// takes a package written a moment before its record for one left by a crash (the review of
    /// task T053).
    pub fn hold(&self) -> Result<fs::File, RuntimeError> {
        crate::records::lock(&self.host.join("install"), true).map_err(|(path, error)| {
            RuntimeError::new(
                ErrorKind::StorageFailed,
                format!("the lock {} could not be taken: {error}", path.display()),
            )
        })
    }

    fn package_dir(&self, package_digest: &str) -> PathBuf {
        self.packages().join(record::directory(package_digest))
    }

    /// The package and the compiled form an installed plugin's record names, as they are on
    /// disk; their digests are checked by [`Runtime::load_compiled`].
    pub fn read(&self, installed: &Installed) -> io::Result<(Vec<u8>, Vec<u8>)> {
        let dir = self.package_dir(&installed.package_digest);
        Ok((
            fs::read(dir.join("package.wasm"))?,
            fs::read(dir.join("compiled.cwasm"))?,
        ))
    }

    /// Writes a package and its compiled form under `packages/<digest>/`, whole or not at all:
    /// both go to a directory of their own first, renamed into place. Returns the compiled
    /// form's digest.
    pub fn write(&self, package: &[u8], compiled: &[u8]) -> io::Result<String> {
        let digest = package::digest(package);
        let packages = self.packages();
        fs::create_dir_all(&packages)?;
        let staging = tempfile::Builder::new()
            .prefix(".staging-")
            .tempdir_in(&packages)?;
        fs::write(staging.path().join("package.wasm"), package)?;
        fs::write(staging.path().join("compiled.cwasm"), compiled)?;
        let target = self.package_dir(&digest);
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        fs::rename(staging.keep(), &target)?;
        Ok(package::digest(compiled))
    }

    /// Replaces an installed package's compiled form, after the package was compiled again.
    pub fn replace_compiled(&self, installed: &Installed, compiled: &[u8]) -> io::Result<String> {
        let dir = self.package_dir(&installed.package_digest);
        crate::records::write_atomic(&dir.join("compiled.cwasm"), compiled)?;
        Ok(package::digest(compiled))
    }

    /// Removes what no record names, so a crash between two writes leaves nothing a plugin can
    /// use (research R5): package directories no installed entry names, installed entries with no
    /// grant from their repository, and repository grants with no installed entry. When either
    /// record cannot be read, nothing is removed.
    pub fn cleanup(&self) -> Result<Cleanup, RuntimeError> {
        let _held = self.hold()?;
        let installed_store = self.installed();
        let installed = installed_store.read()?;
        let grants = self.grants();
        let entries = grants.entries()?;
        let mut removed = Cleanup::default();

        for (id, entry) in &installed {
            let granted = entries
                .get(id)
                .is_some_and(|grant| grant.origin == entry.origin());
            if !granted {
                installed_store.remove(id)?;
                removed.installed.push(id.clone());
            }
        }
        let installed = installed_store.read()?;
        for (id, entry) in &entries {
            let from_repository = entry.origin.starts_with("github:");
            let recorded = installed
                .get(id)
                .is_some_and(|installed| installed.origin() == entry.origin);
            if from_repository && !recorded {
                let id = PluginId::parse(id)
                    .map_err(|reason| RuntimeError::new(ErrorKind::NotGranted, reason))?;
                grants.remove(&id)?;
                removed.grants.push(id.to_string());
            }
        }
        let named: BTreeSet<String> = installed.values().map(Installed::directory).collect();
        if let Ok(dirs) = fs::read_dir(self.packages()) {
            for dir in dirs.flatten() {
                let name = dir.file_name().to_string_lossy().into_owned();
                if !named.contains(&name) {
                    let _ = fs::remove_dir_all(dir.path());
                    removed.packages.push(name);
                }
            }
        }
        Ok(removed)
    }
}

/// A release fetched, compiled and checked, waiting for the person's decision. It lives in
/// memory only.
pub struct Prepared {
    pub kind: DialogKind,
    pub identity: Identity,
    pub release: Release,
    package: Vec<u8>,
    compiled: Vec<u8>,
    pub checked: CheckedPackage,
    /// For an update: the installed version, its record and its checked package.
    previous: Option<(Installed, CheckedPackage)>,
    /// The account sources the installed version's store may hold.
    remembered: Vec<String>,
    /// For an update that is only confirmed: the optional connectors on in the installed
    /// version's grant, which the confirmation keeps.
    kept_on: Vec<String>,
}

impl Prepared {
    /// The plugin's id, which is the repository's.
    pub fn id(&self) -> String {
        self.identity.repository.plugin_id()
    }

    /// The installed version this would replace, for an update.
    pub fn previous(&self) -> Option<&Installed> {
        self.previous.as_ref().map(|(installed, _)| installed)
    }
}

/// The installer, on one runtime, release source, compiler and layout.
pub struct Installer<'a> {
    pub runtime: &'a Runtime,
    pub source: &'a dyn ReleaseSource,
    pub compiler: &'a Compiler,
    pub layout: &'a Layout,
    /// Set to stop a compile in progress: the window closed or the app quits (FR-023).
    pub cancel: Option<&'a AtomicBool>,
}

impl Installer<'_> {
    /// The repository an address names: `github.com/owner/repo` or `https://github.com/owner/repo`,
    /// in any case, with or without a trailing `/` or `.git`, at most 200 characters. Anything
    /// else (a file, another host, another form) is `None`.
    pub fn parse_address(address: &str) -> Option<Repository> {
        if address.chars().count() > 200 {
            return None;
        }
        let trimmed = address.trim();
        let lower = trimmed.to_ascii_lowercase();
        let rest = lower
            .strip_prefix("https://github.com/")
            .or_else(|| lower.strip_prefix("github.com/"))?;
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        let rest = rest.strip_suffix(".git").unwrap_or(rest);
        Repository::from_plugin_id(&format!("github.com/{rest}"))
    }

    /// Fetches, compiles and checks the latest release of `repository`, to install it.
    pub fn prepare_install(&self, repository: &Repository) -> Result<Prepared, RuntimeError> {
        let identity = self.source.identity(repository)?;
        let release = self.source.latest_release(&identity)?;
        let package = self.source.download(&identity, &release)?;
        self.fetched(identity, release, package, None)
    }

    /// The newer release of the installed plugin `id`, fetched by its permanent identifier only;
    /// `None` when there is no newer one.
    pub fn prepare_update(
        &self,
        id: &str,
        installed: &Installed,
    ) -> Result<Option<Prepared>, RuntimeError> {
        let identity = self.source.identity_by_id(installed.repository_id)?;
        if identity.repository.plugin_id() != id {
            return Err(RuntimeError::new(
                ErrorKind::RepositoryChanged,
                format!(
                    "{id} is now {}: a renamed repository is a new plugin; install it again from its new address",
                    identity.repository.plugin_id()
                ),
            ));
        }
        let release = self.source.latest_release(&identity)?;
        let current = semver::Version::parse(&installed.version).map_err(|error| {
            RuntimeError::new(
                ErrorKind::NotGranted,
                format!("the installed version: {error}"),
            )
        })?;
        if release.tag == installed.tag {
            // The pinned release: the same bytes, or it changed under its tag
            let same = match &release.asset_digest {
                Some(digest) => *digest == installed.package_digest,
                None => {
                    package::digest(&self.source.download(&identity, &release)?)
                        == installed.package_digest
                }
            };
            if !same {
                return Err(changed_release(&release));
            }
            return Ok(None);
        }
        if release.version <= current {
            return Err(RuntimeError::new(
                ErrorKind::NotNewer,
                format!(
                    "the latest release, {}, is not newer than the installed {}",
                    release.version, installed.version
                ),
            ));
        }
        let package = self.source.download(&identity, &release)?;
        let (old_package, old_compiled) = self.layout.read(installed).map_err(|error| {
            RuntimeError::new(
                ErrorKind::PackageChanged,
                format!("the installed package: {error}"),
            )
        })?;
        if package::digest(&old_package) != installed.package_digest
            || package::digest(&old_compiled) != installed.compiled_digest
        {
            return Err(RuntimeError::new(
                ErrorKind::PackageChanged,
                "the installed package no longer matches its record",
            ));
        }
        let previous = self.runtime.check_compiled(&old_package, &old_compiled)?;
        self.fetched(
            identity,
            release,
            package,
            Some((installed.clone(), previous)),
        )
        .map(Some)
    }

    fn fetched(
        &self,
        identity: Identity,
        release: Release,
        package: Vec<u8>,
        previous: Option<(Installed, CheckedPackage)>,
    ) -> Result<Prepared, RuntimeError> {
        let compiled = match self.cancel {
            Some(cancel) => self.compiler.compile_until(&package, cancel)?,
            None => self.compiler.compile(&package)?,
        };
        let checked = self.runtime.check_compiled(&package, &compiled)?;
        let manifest = checked.manifest();
        let id = identity.repository.plugin_id();
        if manifest.id().as_str() != id {
            return Err(RuntimeError::new(
                ErrorKind::IdentityMismatch,
                format!(
                    "the package's manifest names {}, but it was downloaded from {id}",
                    manifest.id()
                ),
            ));
        }
        let version = semver::Version::parse(manifest.version()).map_err(|error| {
            RuntimeError::new(ErrorKind::InvalidManifest, format!("version: {error}"))
        })?;
        if version != release.version {
            return Err(RuntimeError::new(
                ErrorKind::TagMismatch,
                format!(
                    "the release is tagged {} but its manifest says version {}",
                    release.tag,
                    manifest.version()
                ),
            ));
        }
        let origin = record::origin(identity.repository_id);
        let entry = self
            .layout
            .grants()
            .entry(manifest.id())?
            .filter(|entry| entry.origin == origin);
        let kind = match (&previous, &entry) {
            (None, _) => DialogKind::Install,
            (Some(_), Some(entry)) if !checked.needs_new_approval(entry) => {
                DialogKind::UpdateConfirm
            }
            (Some(_), _) => DialogKind::Update,
        };
        let kept_on = match (&kind, &entry) {
            (DialogKind::UpdateConfirm, Some(entry)) => manifest
                .connectors()
                .iter()
                .filter(|request| !request.required && entry.is_connector_granted(&request.id))
                .map(|request| request.id.clone())
                .collect(),
            _ => Vec::new(),
        };
        let remembered = entry.map(|entry| entry.account_sources).unwrap_or_default();
        Ok(Prepared {
            kind,
            identity,
            release,
            package,
            compiled,
            checked,
            previous,
            remembered,
            kept_on,
        })
    }

    /// The dialog's model of `prepared`, with `optional_on` turned on.
    pub fn model(&self, prepared: &Prepared, optional_on: &[String]) -> DialogModel {
        model(&ModelInput {
            kind: prepared.kind,
            package: &prepared.checked,
            previous: prepared.previous.as_ref().map(|(_, checked)| checked),
            optional_on,
            remembered: &prepared.remembered,
            kept_on: &prepared.kept_on,
        })
    }

    /// Installs `prepared`, when `choice` is the choice digest of what the dialog showed for
    /// `optional_on`: the package and its compiled form, then the grant, then the installed
    /// record. Anything else is `approval-changed`, and nothing is written.
    pub fn commit(
        &self,
        prepared: &Prepared,
        choice: &str,
        optional_on: &[String],
    ) -> Result<Installed, RuntimeError> {
        let shown = self.model(prepared, optional_on);
        if shown.choice != choice {
            return Err(RuntimeError::new(
                ErrorKind::ApprovalChanged,
                "what would be approved is not what the dialog showed; show it again",
            ));
        }
        if prepared.kind == DialogKind::UpdateConfirm && !optional_on.is_empty() {
            return Err(RuntimeError::new(
                ErrorKind::ApprovalChanged,
                "an update that needs nothing new turns nothing on",
            ));
        }
        let unwritable = |error: io::Error| {
            RuntimeError::new(
                ErrorKind::StorageFailed,
                format!("the package could not be written: {error}"),
            )
        };
        let _held = self.layout.hold()?;
        let compiled_digest = self
            .layout
            .write(&prepared.package, &prepared.compiled)
            .map_err(unwritable)?;
        let origin = record::origin(prepared.identity.repository_id);
        let grants = self.layout.grants();
        match prepared.kind {
            DialogKind::UpdateConfirm => {
                prepared
                    .checked
                    .approve(&grants, &origin, Some(&shown.digest))?;
            }
            DialogKind::Install | DialogKind::Update => {
                prepared
                    .checked
                    .install(&grants, &origin, &shown.digest, optional_on)?;
            }
        }
        let installed = Installed {
            repository_id: prepared.identity.repository_id,
            tag: prepared.release.tag.clone(),
            version: prepared.checked.manifest().version().to_owned(),
            package_digest: package::digest(&prepared.package),
            compiled_digest,
            installed_at: now_rfc3339(),
        };
        self.layout
            .installed()
            .insert(&prepared.id(), installed.clone())?;
        Ok(installed)
    }

    /// Loads the installed plugin `id` from its files, checking both digests. A compiled form
    /// that changed, or that this app's engine refuses (an app update), is compiled again from
    /// the package, once; a package that changed is refused.
    pub fn load(
        &self,
        id: &str,
        installed: &Installed,
        grants: Arc<GrantStore>,
    ) -> Result<Plugin, RuntimeError> {
        let (package, compiled) = self.layout.read(installed).map_err(|error| {
            RuntimeError::new(
                ErrorKind::PackageChanged,
                format!("the installed package: {error}"),
            )
        })?;
        let origin = installed.origin();
        let first = self.runtime.load_compiled(
            &package,
            &installed.package_digest,
            &compiled,
            &installed.compiled_digest,
            &origin,
            Arc::clone(&grants),
        );
        match first {
            Err(error) if error.kind() == ErrorKind::CompileFailed => {
                let compiled = self.compiler.compile(&package)?;
                let compiled_digest =
                    self.layout
                        .replace_compiled(installed, &compiled)
                        .map_err(|error| {
                            RuntimeError::new(
                                ErrorKind::StorageFailed,
                                format!("the compiled form could not be written: {error}"),
                            )
                        })?;
                let mut recorded = installed.clone();
                recorded.compiled_digest = compiled_digest.clone();
                self.layout.installed().insert(id, recorded)?;
                self.runtime.load_compiled(
                    &package,
                    &installed.package_digest,
                    &compiled,
                    &compiled_digest,
                    &origin,
                    grants,
                )
            }
            other => other,
        }
    }
}

fn changed_release(release: &Release) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::ChangedRelease,
        format!(
            "the release {} now carries other bytes than the ones installed",
            release.tag
        ),
    )
}

/// Now, as RFC 3339 in UTC (`2026-11-12T20:00:00Z`).
fn now_rfc3339() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let days = seconds / 86_400;
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The date of a count of days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_civil() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_404), (2025, 11, 12));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    #[test]
    fn addresses_name_one_repository_or_none() {
        let ok = |address: &str| {
            Installer::parse_address(address).map(|repository| repository.plugin_id())
        };
        for address in [
            "github.com/Owner/Repo",
            "https://github.com/owner/repo",
            "https://github.com/owner/repo/",
            "github.com/owner/repo.git",
            "  github.com/owner/repo  ",
        ] {
            assert_eq!(
                ok(address).as_deref(),
                Some("github.com/owner/repo"),
                "{address}"
            );
        }
        for address in [
            "file:///tmp/x.wasm",
            "/tmp/x.wasm",
            "https://example.com/x",
            "gitlab.com/a/b",
            "http://github.com/owner/repo",
            "github.com/owner",
            "github.com/owner/repo/releases",
            "github.com/../repo",
            "",
        ] {
            assert_eq!(ok(address), None, "{address}");
        }
        assert_eq!(ok(&format!("github.com/owner/{}", "r".repeat(200))), None);
    }
}
