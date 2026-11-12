//! Runtime errors. Each kind names the rule that refused, stopped or failed a plugin, so the
//! maintainer knows which one applied without reading source code (SC-004).

use std::fmt;

use crate::text;

/// Declares [`ErrorKind`], its names and [`ErrorKind::ALL`] from one list, so no kind can exist
/// outside `ALL` (`specs/006-contract-v1-docs/research.md`, R7).
macro_rules! error_kinds {
    ($($(#[$doc:meta])* $kind:ident => $name:literal,)*) => {
        /// The rule behind a [`RuntimeError`]. The names are part of the CLI contract
        /// (`specs/001-plugin-runtime/contracts/cli.md`,
        /// `specs/002-plugin-permissions/contracts/cli.md`,
        /// `specs/003-plugin-connectors/contracts/cli.md`,
        /// `specs/005-plugin-credentials/contracts/cli.md`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ErrorKind {
            $($(#[$doc])* $kind,)*
        }

        impl ErrorKind {
            /// Every kind, in the order they are declared.
            pub const ALL: &'static [ErrorKind] = &[$(ErrorKind::$kind,)*];

            /// The kind's name, as the CLI prints it.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(ErrorKind::$kind => $name,)*
                }
            }
        }
    };
}

error_kinds! {
    /// Not a WebAssembly component, corrupted, or larger than the package limit.
    InvalidPackage => "invalid-package",
    /// Implements no contract version the host runs, or imports from another version than the
    /// one it implements, including a newer minor version.
    ContractMismatch => "contract-mismatch",
    /// Imports something the host does not offer.
    CapabilityRefused => "capability-refused",
    /// The package has no manifest section.
    NoManifest => "no-manifest",
    /// The package has more than one manifest section, or one inside a nested module or
    /// component; or `pack` was given a component that already has one.
    ManifestMismatch => "manifest-mismatch",
    /// The manifest is too large, not JSON, or breaks the schema.
    InvalidManifest => "invalid-manifest",
    /// The manifest requests a permission this host does not know.
    UnknownPermission => "unknown-permission",
    /// A contract v0 plugin marks a permission optional, which only v1 can answer.
    OptionalNeedsV1 => "optional-needs-v1",
    /// The component imports a capability its manifest does not declare.
    NotDeclared => "not-declared",
    /// The plugin has no approval for this package, or the grant record cannot be read.
    NotGranted => "not-granted",
    /// An earlier version was approved, and this one requires permissions not granted.
    NeedsNewApproval => "needs-new-approval",
    /// A required permission was turned on or off alone: required permissions change only
    /// together.
    RequiredTogether => "required-together",
    /// Ran out of fuel, the CPU budget.
    CpuBudgetExceeded => "cpu-budget-exceeded",
    /// Hit the wall-clock time limit.
    TimeLimitExceeded => "time-limit-exceeded",
    /// Tried to grow memory beyond the cap.
    MemoryLimitExceeded => "memory-limit-exceeded",
    /// Any other trap.
    PluginCrashed => "plugin-crashed",
    /// `render` returned an error.
    PluginError => "plugin-error",
    /// The widget content is not valid JSON.
    InvalidOutput => "invalid-output",
    /// The widget content exceeds the size cap.
    OutputTooLarge => "output-too-large",
    /// A connector file is too large, not JSON, or breaks the connector schema or a rule of the
    /// host.
    InvalidConnector => "invalid-connector",
    /// A connector declares a credential under a field other than `credential` (`credentials`
    /// or `auth`).
    CredentialsNotSupported => "credentials-not-supported",
    /// A connector's id names a repository, whose destinations nothing verifies yet, or is a
    /// local id while the runtime is not in development mode.
    DestinationNotVerified => "destination-not-verified",
    /// A connector outside the catalog claims a catalog id or a host the catalog reserves.
    DestinationReserved => "destination-reserved",
    /// A declared connector is not found, has another major version, or lacks an operation the
    /// widget declares.
    ConnectorUnavailable => "connector-unavailable",
    /// A widget declares a connector or storage but is built for a contract older than the one
    /// that added it (1.1.0 and 1.2.0).
    ContractTooOld => "contract-too-old",
    /// A widget declares a connector or storage but exports no `updater`, or `update` was asked
    /// of a widget without one.
    MissingUpdater => "missing-updater",
    /// A control asked for an action the manifest does not declare, or a plugin built before
    /// contract 1.4.0 was asked for one; the plugin was not called (feature 009).
    UndeclaredAction => "undeclared-action",
    /// An approval was given the digest of something other than what it would approve now.
    ApprovalChanged => "approval-changed",
    /// An update's storage could not be committed (busy, damaged, unavailable, the file cap, or
    /// a run an earlier failure ended), or required storage is unavailable at load.
    StorageFailed => "storage-failed",
    /// A connector that declares a credential has a destination without HTTPS: only a connector
    /// that sends no credential may go without it.
    CredentialNeedsHttps => "credential-needs-https",
    /// A widget declares a connector whose credential is required without `"account": true`.
    AccountUseNotDeclared => "account-use-not-declared",
    /// A sign-in ended without connecting an account: a wrong or missing state, a code already
    /// used, the time limit, a token outside the bearer-token set, or the service's error.
    SignInFailed => "sign-in-failed",
    /// Another account, or another key, is connected to the connector: disconnect it first.
    AnotherAccountConnected => "another-account-connected",
    /// A key typed for a connector is empty, too long or holds a character a bearer token
    /// cannot.
    KeyRefused => "key-refused",
    /// The system's keychain is locked, missing or did not answer in time.
    KeychainUnavailable => "keychain-unavailable",
    /// A connector's account needs a client registration the host has none of; `--client`
    /// gives one in development.
    NoClientConfigured => "no-client-configured",
    /// No account is connected to the connector, or it must be connected again.
    AccountNotConnected => "account-not-connected",
    /// The package's manifest names another repository than the one it was installed from
    /// (feature 010).
    IdentityMismatch => "identity-mismatch",
    /// A release's tag is not `v<version>` or `<version>` of the manifest's version.
    TagMismatch => "tag-mismatch",
    /// A release the host pinned now carries other bytes under the same tag.
    ChangedRelease => "changed-release",
    /// The repository an installed plugin came from was deleted or renamed, or its name now
    /// belongs to another repository; no update is taken from it.
    RepositoryChanged => "repository-changed",
    /// An update's version is not greater than the installed one.
    NotNewer => "not-newer",
    /// The code host could not be reached, or answered something the host does not accept.
    SourceUnreachable => "source-unreachable",
    /// The code host's request limit was reached; the detail says when to try again.
    RateLimited => "rate-limited",
    /// A release has no `.wasm` asset, or more than one, or one larger than the package limit.
    NoPackageAsset => "no-package-asset",
    /// Compiling the package reached the compile process's time limit.
    CompileTimeLimit => "compile-time-limit",
    /// Compiling the package reached the compile process's memory limit.
    CompileMemoryLimit => "compile-memory-limit",
    /// The compile process failed otherwise, or the app was updated while it ran.
    CompileFailed => "compile-failed",
    /// An installed package's bytes no longer match the digest recorded at installation.
    PackageChanged => "package-changed",
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a plugin was refused, stopped or failed, displayed as `<kind>: <detail>`.
///
/// Only the runtime builds these, and only through the constructors below, which escape every
/// detail: plugin messages, compile errors, trap descriptions, import names, permission names
/// and manifest fields can all quote the plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    kind: ErrorKind,
    detail: String,
    names: Vec<String>,
}

/// Longest detail kept, and longest name in a list, before the ` [truncated]` marker.
const MAX_DETAIL_BYTES: usize = 1024;

impl RuntimeError {
    pub(crate) fn new(kind: ErrorKind, detail: impl AsRef<str>) -> Self {
        Self {
            kind,
            detail: text::untrusted(detail.as_ref(), MAX_DETAIL_BYTES),
            names: Vec::new(),
        }
    }

    /// An error that lists every name it refuses (imports, permissions), with an optional
    /// explanation after them: `<names> (<explanation>)`.
    pub(crate) fn with_names(
        kind: ErrorKind,
        names: Vec<String>,
        explanation: Option<&str>,
    ) -> Self {
        let names: Vec<String> = names
            .iter()
            .map(|name| text::untrusted(name, MAX_DETAIL_BYTES))
            .collect();
        let list = names.join(", ");
        let detail = match explanation {
            None => list,
            Some(explanation) if list.is_empty() => explanation.to_owned(),
            Some(explanation) => format!("{list} ({explanation})"),
        };
        Self {
            kind,
            // The names are already escaped, and escaping leaves no character to escape again.
            detail: text::untrusted(&detail, MAX_DETAIL_BYTES),
            names,
        }
    }
}

impl RuntimeError {
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// The names the error lists, escaped: refused imports, undeclared or ungranted
    /// permissions. Empty for errors that list none.
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for RuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_names_match_the_cli_contract() {
        let kinds = [
            (ErrorKind::InvalidPackage, "invalid-package"),
            (ErrorKind::ContractMismatch, "contract-mismatch"),
            (ErrorKind::CapabilityRefused, "capability-refused"),
            (ErrorKind::NoManifest, "no-manifest"),
            (ErrorKind::ManifestMismatch, "manifest-mismatch"),
            (ErrorKind::InvalidManifest, "invalid-manifest"),
            (ErrorKind::UnknownPermission, "unknown-permission"),
            (ErrorKind::OptionalNeedsV1, "optional-needs-v1"),
            (ErrorKind::NotDeclared, "not-declared"),
            (ErrorKind::NotGranted, "not-granted"),
            (ErrorKind::NeedsNewApproval, "needs-new-approval"),
            (ErrorKind::RequiredTogether, "required-together"),
            (ErrorKind::CpuBudgetExceeded, "cpu-budget-exceeded"),
            (ErrorKind::TimeLimitExceeded, "time-limit-exceeded"),
            (ErrorKind::MemoryLimitExceeded, "memory-limit-exceeded"),
            (ErrorKind::PluginCrashed, "plugin-crashed"),
            (ErrorKind::PluginError, "plugin-error"),
            (ErrorKind::InvalidOutput, "invalid-output"),
            (ErrorKind::OutputTooLarge, "output-too-large"),
            (ErrorKind::InvalidConnector, "invalid-connector"),
            (
                ErrorKind::CredentialsNotSupported,
                "credentials-not-supported",
            ),
            (
                ErrorKind::DestinationNotVerified,
                "destination-not-verified",
            ),
            (ErrorKind::DestinationReserved, "destination-reserved"),
            (ErrorKind::ConnectorUnavailable, "connector-unavailable"),
            (ErrorKind::ContractTooOld, "contract-too-old"),
            (ErrorKind::MissingUpdater, "missing-updater"),
            (ErrorKind::UndeclaredAction, "undeclared-action"),
            (ErrorKind::ApprovalChanged, "approval-changed"),
            (ErrorKind::StorageFailed, "storage-failed"),
            (ErrorKind::CredentialNeedsHttps, "credential-needs-https"),
            (ErrorKind::AccountUseNotDeclared, "account-use-not-declared"),
            (ErrorKind::SignInFailed, "sign-in-failed"),
            (
                ErrorKind::AnotherAccountConnected,
                "another-account-connected",
            ),
            (ErrorKind::KeyRefused, "key-refused"),
            (ErrorKind::KeychainUnavailable, "keychain-unavailable"),
            (ErrorKind::NoClientConfigured, "no-client-configured"),
            (ErrorKind::AccountNotConnected, "account-not-connected"),
            (ErrorKind::IdentityMismatch, "identity-mismatch"),
            (ErrorKind::TagMismatch, "tag-mismatch"),
            (ErrorKind::ChangedRelease, "changed-release"),
            (ErrorKind::RepositoryChanged, "repository-changed"),
            (ErrorKind::NotNewer, "not-newer"),
            (ErrorKind::SourceUnreachable, "source-unreachable"),
            (ErrorKind::RateLimited, "rate-limited"),
            (ErrorKind::NoPackageAsset, "no-package-asset"),
            (ErrorKind::CompileTimeLimit, "compile-time-limit"),
            (ErrorKind::CompileMemoryLimit, "compile-memory-limit"),
            (ErrorKind::CompileFailed, "compile-failed"),
            (ErrorKind::PackageChanged, "package-changed"),
        ];
        for (kind, name) in kinds {
            assert_eq!(kind.as_str(), name);
        }
    }

    #[test]
    fn all_lists_every_kind_once() {
        let names: std::collections::BTreeSet<&str> =
            ErrorKind::ALL.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(ErrorKind::ALL.len(), 49);
        assert_eq!(names.len(), ErrorKind::ALL.len());
    }

    #[test]
    fn displays_kind_then_detail() {
        let error = RuntimeError::new(ErrorKind::PluginError, "no name given");
        assert_eq!(error.to_string(), "plugin-error: no name given");
    }

    #[test]
    fn escapes_and_caps_every_detail() {
        let error = RuntimeError::new(ErrorKind::PluginError, "\nerror: forged");
        assert_eq!(error.detail(), r"\nerror: forged");

        let error = RuntimeError::new(ErrorKind::PluginError, "x".repeat(4096));
        assert_eq!(
            error.detail().len(),
            MAX_DETAIL_BYTES + " [truncated]".len()
        );

        let error = RuntimeError::with_names(
            ErrorKind::CapabilityRefused,
            vec!["url=<a\nb>".to_owned()],
            None,
        );
        assert_eq!(error.names(), [r"url=<a\nb>"]);
        assert_eq!(error.detail(), r"url=<a\nb>");
    }

    #[test]
    fn with_names_lists_every_name() {
        let imports = vec![
            "wasi:filesystem/types@0.2.0".to_owned(),
            "wasi:sockets/tcp@0.2.0".to_owned(),
        ];
        let error = RuntimeError::with_names(ErrorKind::CapabilityRefused, imports.clone(), None);
        assert_eq!(error.kind(), ErrorKind::CapabilityRefused);
        assert_eq!(error.names(), imports);
        assert_eq!(
            error.to_string(),
            "capability-refused: wasi:filesystem/types@0.2.0, wasi:sockets/tcp@0.2.0"
        );
    }

    #[test]
    fn with_names_adds_the_explanation_after_the_names() {
        let error = RuntimeError::with_names(
            ErrorKind::NotGranted,
            vec!["log".to_owned()],
            Some("local/probe has no approval"),
        );
        assert_eq!(
            error.to_string(),
            "not-granted: log (local/probe has no approval)"
        );

        let error = RuntimeError::with_names(
            ErrorKind::NotGranted,
            Vec::new(),
            Some("local/probe has no approval"),
        );
        assert_eq!(error.detail(), "local/probe has no approval");
        assert!(error.names().is_empty());
    }

    #[test]
    fn with_names_escapes_the_explanation_and_caps_the_detail() {
        let error = RuntimeError::with_names(
            ErrorKind::NotGranted,
            vec!["log".to_owned()],
            Some("approved from /tmp/a\nerror: forged"),
        );
        assert_eq!(error.detail(), r"log (approved from /tmp/a\nerror: forged)");

        let names = vec!["x".repeat(800), "y".repeat(800)];
        let error = RuntimeError::with_names(ErrorKind::NotDeclared, names, None);
        assert_eq!(
            error.detail().len(),
            MAX_DETAIL_BYTES + " [truncated]".len()
        );
        assert_eq!(error.names().len(), 2);
    }
}
