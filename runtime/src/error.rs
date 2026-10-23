//! Runtime errors. Each kind names the rule that refused, stopped or failed a plugin, so the
//! maintainer knows which one applied without reading source code (SC-004).

use std::fmt;

use crate::text;

/// The rule behind a [`RuntimeError`]. The names are part of the CLI contract
/// (`specs/001-plugin-runtime/contracts/cli.md`, `specs/002-plugin-permissions/contracts/cli.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Not a WebAssembly component, corrupted, or larger than the package limit.
    InvalidPackage,
    /// Implements no contract version the host runs, or imports from another version than the
    /// one it implements, including a newer minor version.
    ContractMismatch,
    /// Imports something the host does not offer.
    CapabilityRefused,
    /// The package has no manifest section.
    NoManifest,
    /// The package has more than one manifest section, or one inside a nested module or
    /// component; or `pack` was given a component that already has one.
    ManifestMismatch,
    /// The manifest is too large, not JSON, or breaks the schema.
    InvalidManifest,
    /// The manifest requests a permission this host does not know.
    UnknownPermission,
    /// A contract v0 plugin marks a permission optional, which only v1 can answer.
    OptionalNeedsV1,
    /// The component imports a capability its manifest does not declare.
    NotDeclared,
    /// The plugin has no approval for this package, or the grant record cannot be read.
    NotGranted,
    /// An earlier version was approved, and this one requires permissions not granted.
    NeedsNewApproval,
    /// A required permission was turned on or off alone: required permissions change only
    /// together.
    RequiredTogether,
    /// Ran out of fuel, the CPU budget.
    CpuBudgetExceeded,
    /// Hit the wall-clock time limit.
    TimeLimitExceeded,
    /// Tried to grow memory beyond the cap.
    MemoryLimitExceeded,
    /// Any other trap.
    PluginCrashed,
    /// `render` returned an error.
    PluginError,
    /// The widget content is not valid JSON.
    InvalidOutput,
    /// The widget content exceeds the size cap.
    OutputTooLarge,
}

impl ErrorKind {
    /// The kind's name, as the CLI prints it.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::InvalidPackage => "invalid-package",
            ErrorKind::ContractMismatch => "contract-mismatch",
            ErrorKind::CapabilityRefused => "capability-refused",
            ErrorKind::NoManifest => "no-manifest",
            ErrorKind::ManifestMismatch => "manifest-mismatch",
            ErrorKind::InvalidManifest => "invalid-manifest",
            ErrorKind::UnknownPermission => "unknown-permission",
            ErrorKind::OptionalNeedsV1 => "optional-needs-v1",
            ErrorKind::NotDeclared => "not-declared",
            ErrorKind::NotGranted => "not-granted",
            ErrorKind::NeedsNewApproval => "needs-new-approval",
            ErrorKind::RequiredTogether => "required-together",
            ErrorKind::CpuBudgetExceeded => "cpu-budget-exceeded",
            ErrorKind::TimeLimitExceeded => "time-limit-exceeded",
            ErrorKind::MemoryLimitExceeded => "memory-limit-exceeded",
            ErrorKind::PluginCrashed => "plugin-crashed",
            ErrorKind::PluginError => "plugin-error",
            ErrorKind::InvalidOutput => "invalid-output",
            ErrorKind::OutputTooLarge => "output-too-large",
        }
    }
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
        ];
        for (kind, name) in kinds {
            assert_eq!(kind.as_str(), name);
        }
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
