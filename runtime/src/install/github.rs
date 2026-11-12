//! Where releases come from (`specs/010-plugin-installation/research.md`, R4): the
//! [`ReleaseSource`] the installer asks, and [`GitHub`], the only one a release build has.
//!
//! A repository is named once, when the person types its address; from then on everything is
//! asked by its permanent identifier, which survives renames and is never reused, so a name that
//! changes hands between two requests, or after the installation, cannot serve another
//! repository's release (repo-jacking). Every answer is read as untrusted: capped, parsed, and
//! refused with a named rule when it is not what the installer accepts.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::{FetchLimits, Repository, unreachable_source};
use crate::network::{Failure, FailureRule, HostGet, Network};
use crate::{ErrorKind, Limits, RuntimeError, package, text};

/// GitHub's two hosts a package download may be redirected to, and no other.
pub const ASSET_HOSTS: [&str; 2] = [
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];

/// The API's version this host is written against.
const API_VERSION: &str = "2022-11-28";

/// A repository and its permanent identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub repository: Repository,
    /// GitHub's identifier of the repository: it survives renames and is never given to another.
    pub repository_id: u64,
}

/// The latest published release of a repository, and its one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    /// The tag's version, without its `v`.
    pub version: semver::Version,
    pub asset_id: u64,
    pub asset_name: String,
    /// The size GitHub declares, already checked against the package cap.
    pub asset_size: u64,
    /// `sha256:<hex>`, when GitHub gives one; the download must match it.
    pub asset_digest: Option<String>,
    /// True when the release was found by name, because GitHub did not answer by identifier:
    /// the download then checks again that the name still leads to the same repository.
    pub found_by_name: bool,
}

/// What the installer asks for releases. Release builds have [`GitHub`] alone; the runtime's
/// tests and the probe build have sources of their own (research R16).
pub trait ReleaseSource: Send + Sync {
    /// The repository a typed address names, with its permanent identifier.
    fn identity(&self, repository: &Repository) -> Result<Identity, RuntimeError>;
    /// The repository with this identifier now: for updates, which never go by name.
    fn identity_by_id(&self, repository_id: u64) -> Result<Identity, RuntimeError>;
    /// The latest published release of the repository.
    fn latest_release(&self, identity: &Identity) -> Result<Release, RuntimeError>;
    /// The release's package, checked against GitHub's digest when there is one.
    fn download(&self, identity: &Identity, release: &Release) -> Result<Vec<u8>, RuntimeError>;
}

/// The GitHub REST API, without an account.
pub struct GitHub {
    network: Network,
    limits: Limits,
    fetch: FetchLimits,
    api: url::Url,
}

impl GitHub {
    pub(crate) fn new(network: Network, limits: Limits, fetch: FetchLimits) -> GitHub {
        GitHub {
            network,
            limits,
            fetch,
            api: url::Url::parse("https://api.github.com").expect("a valid address"),
        }
    }

    /// `GET` an API path, following no redirect; the rate limit is refused here.
    fn api_get(&self, path: &str) -> Result<HostGet, RuntimeError> {
        let url = self
            .api
            .join(path)
            .map_err(|error| unreachable_source(format!("{path}: {error}")))?;
        let answer = self
            .network
            .session(&self.limits)
            .host_get(
                &url,
                &[
                    ("Accept", "application/vnd.github+json"),
                    ("X-GitHub-Api-Version", API_VERSION),
                ],
                &[],
                0,
                self.fetch.api_time,
                self.fetch.api_body_bytes,
            )
            .map_err(|failure| from_failure(&failure))?;
        rate_limit(&answer)?;
        Ok(answer)
    }

    fn json(answer: &HostGet, what: &str) -> Result<Value, RuntimeError> {
        serde_json::from_slice(&answer.body).map_err(|error| {
            unreachable_source(format!("GitHub's answer for {what} is not JSON: {error}"))
        })
    }

    fn identity_of(&self, body: &Value) -> Result<Identity, RuntimeError> {
        let repository_id = body["id"]
            .as_u64()
            .ok_or_else(|| unreachable_source("GitHub's answer has no repository id"))?;
        let full_name = body["full_name"]
            .as_str()
            .ok_or_else(|| unreachable_source("GitHub's answer has no repository name"))?;
        let id = format!("github.com/{}", full_name.to_ascii_lowercase());
        let repository = Repository::from_plugin_id(&id).ok_or_else(|| {
            unreachable_source(format!(
                "GitHub names the repository {}, which is not a plugin id",
                text::untrusted(full_name, 200)
            ))
        })?;
        Ok(Identity {
            repository,
            repository_id,
        })
    }

    fn release_of(&self, body: &Value, found_by_name: bool) -> Result<Release, RuntimeError> {
        if body["draft"].as_bool() == Some(true) || body["prerelease"].as_bool() == Some(true) {
            return Err(unreachable_source(
                "GitHub answered a draft or a prerelease as the latest release",
            ));
        }
        let tag = body["tag_name"]
            .as_str()
            .ok_or_else(|| unreachable_source("the release has no tag"))?;
        let version =
            semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag)).map_err(|_| {
                RuntimeError::new(
                    ErrorKind::TagMismatch,
                    format!(
                        "the release's tag {} is not v<version> or <version>",
                        text::untrusted(tag, 200)
                    ),
                )
            })?;
        let assets = body["assets"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        let packages: Vec<&Value> = assets
            .iter()
            .filter(|asset| {
                asset["name"]
                    .as_str()
                    .is_some_and(|name| name.ends_with(".wasm"))
            })
            .collect();
        let shown_tag = text::untrusted(tag, 200);
        let asset = match packages.as_slice() {
            [one] => *one,
            [] => {
                return Err(RuntimeError::new(
                    ErrorKind::NoPackageAsset,
                    format!("the release {shown_tag} has no asset whose name ends in .wasm"),
                ));
            }
            several => {
                return Err(RuntimeError::new(
                    ErrorKind::NoPackageAsset,
                    format!(
                        "the release {shown_tag} has {} assets whose names end in .wasm; it must have one",
                        several.len()
                    ),
                ));
            }
        };
        let asset_id = asset["id"]
            .as_u64()
            .ok_or_else(|| unreachable_source("the package asset has no id"))?;
        let asset_size = asset["size"]
            .as_u64()
            .ok_or_else(|| unreachable_source("the package asset has no size"))?;
        let cap = self.limits.max_package_bytes as u64;
        if asset_size > cap {
            return Err(over_cap(cap));
        }
        let asset_digest = match &asset["digest"] {
            Value::Null => None,
            Value::String(digest) if is_sha256(digest) => Some(digest.to_ascii_lowercase()),
            _ => {
                return Err(unreachable_source(
                    "the package asset's digest is not sha256:<hex>",
                ));
            }
        };
        Ok(Release {
            tag: tag.to_owned(),
            version,
            asset_id,
            asset_name: asset["name"].as_str().unwrap_or_default().to_owned(),
            asset_size,
            asset_digest,
            found_by_name,
        })
    }
}

impl ReleaseSource for GitHub {
    fn identity(&self, repository: &Repository) -> Result<Identity, RuntimeError> {
        let answer = self.api_get(&format!("/repos/{}", repository.full_name()))?;
        match answer.status {
            200 => {}
            301 | 302 | 307 | 308 => {
                return Err(RuntimeError::new(
                    ErrorKind::RepositoryChanged,
                    format!(
                        "{} was renamed or moved; install it from its new address",
                        repository.plugin_id()
                    ),
                ));
            }
            404 => {
                return Err(unreachable_source(format!(
                    "GitHub has no public repository {}",
                    repository.plugin_id()
                )));
            }
            status => return Err(unexpected(status, "the repository")),
        }
        let identity = self.identity_of(&Self::json(&answer, "the repository")?)?;
        if identity.repository != *repository {
            return Err(RuntimeError::new(
                ErrorKind::RepositoryChanged,
                format!(
                    "{} is now {}",
                    repository.plugin_id(),
                    identity.repository.plugin_id()
                ),
            ));
        }
        Ok(identity)
    }

    fn identity_by_id(&self, repository_id: u64) -> Result<Identity, RuntimeError> {
        let answer = self.api_get(&format!("/repositories/{repository_id}"))?;
        match answer.status {
            200 => self.identity_of(&Self::json(&answer, "the repository")?),
            404 => Err(RuntimeError::new(
                ErrorKind::RepositoryChanged,
                "the repository the plugin was installed from was deleted or is no longer public",
            )),
            status => Err(unexpected(status, "the repository")),
        }
    }

    fn latest_release(&self, identity: &Identity) -> Result<Release, RuntimeError> {
        let by_id = format!("/repositories/{}/releases/latest", identity.repository_id);
        let answer = self.api_get(&by_id)?;
        match answer.status {
            200 => return self.release_of(&Self::json(&answer, "the latest release")?, false),
            404 => {}
            status => return Err(unexpected(status, "the latest release")),
        }
        // GitHub serves paths by identifier without documenting them; if it stops, the release is
        // asked by name, and the download checks the name still leads to this repository
        let by_name = format!("/repos/{}/releases/latest", identity.repository.full_name());
        let answer = self.api_get(&by_name)?;
        match answer.status {
            200 => self.release_of(&Self::json(&answer, "the latest release")?, true),
            404 => Err(RuntimeError::new(
                ErrorKind::NoPackageAsset,
                format!(
                    "{} has no published release",
                    identity.repository.plugin_id()
                ),
            )),
            status => Err(unexpected(status, "the latest release")),
        }
    }

    fn download(&self, identity: &Identity, release: &Release) -> Result<Vec<u8>, RuntimeError> {
        let path = if release.found_by_name {
            format!(
                "/repos/{}/releases/assets/{}",
                identity.repository.full_name(),
                release.asset_id
            )
        } else {
            format!(
                "/repositories/{}/releases/assets/{}",
                identity.repository_id, release.asset_id
            )
        };
        let url = self
            .api
            .join(&path)
            .map_err(|error| unreachable_source(format!("{path}: {error}")))?;
        let cap = self.limits.max_package_bytes;
        let answer = self
            .network
            .session(&self.limits)
            .host_get(
                &url,
                &[
                    ("Accept", "application/octet-stream"),
                    ("X-GitHub-Api-Version", API_VERSION),
                ],
                &ASSET_HOSTS,
                self.fetch.max_redirects,
                self.fetch.package_time,
                cap,
            )
            .map_err(|failure| match failure.rule {
                FailureRule::OverLimit if failure.detail.contains("bytes") => over_cap(cap as u64),
                _ => from_failure(&failure),
            })?;
        rate_limit(&answer)?;
        if answer.status != 200 {
            return Err(unexpected(answer.status, "the package"));
        }
        if let Some(expected) = &release.asset_digest
            && package::digest(&answer.body) != *expected
        {
            return Err(unreachable_source(
                "the package downloaded does not match the digest GitHub gives for it",
            ));
        }
        if release.found_by_name {
            let now = self.identity_by_id(identity.repository_id)?;
            if now.repository != identity.repository {
                return Err(RuntimeError::new(
                    ErrorKind::RepositoryChanged,
                    format!(
                        "{} changed while its release was fetched",
                        identity.repository.plugin_id()
                    ),
                ));
            }
        }
        Ok(answer.body)
    }
}

fn is_sha256(digest: &str) -> bool {
    digest
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()))
}

fn over_cap(cap: u64) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::NoPackageAsset,
        format!("the package is larger than the limit of {cap} bytes"),
    )
}

fn unexpected(status: u16, what: &str) -> RuntimeError {
    unreachable_source(format!("GitHub answered {status} for {what}"))
}

fn from_failure(failure: &Failure) -> RuntimeError {
    unreachable_source(format!("GitHub: {}", failure.detail))
}

/// `rate-limited` for an answer that says GitHub's limit was reached: a `403` or `429` with no
/// requests left, or with `retry-after`.
fn rate_limit(answer: &HostGet) -> Result<(), RuntimeError> {
    if !matches!(answer.status, 403 | 429) {
        return Ok(());
    }
    let wait = if let Some(seconds) = answer
        .header("retry-after")
        .and_then(|value| value.trim().parse::<u64>().ok())
    {
        Some(Duration::from_secs(seconds))
    } else if answer.header("x-ratelimit-remaining").map(str::trim) == Some("0") {
        let reset = answer
            .header("x-ratelimit-reset")
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or_default();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        Some(Duration::from_secs(reset.saturating_sub(now)))
    } else {
        None
    };
    match wait {
        Some(wait) => Err(RuntimeError::new(
            ErrorKind::RateLimited,
            format!(
                "GitHub's limit for requests without an account was reached; try again in {} minutes",
                wait.as_secs().div_ceil(60).max(1)
            ),
        )),
        None => Ok(()),
    }
}
