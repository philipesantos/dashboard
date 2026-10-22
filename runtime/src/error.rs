//! Runtime errors. Each kind names the rule that refused, stopped or failed a plugin, so the
//! maintainer knows which one applied without reading source code (SC-004).

use std::fmt;

/// The rule behind a [`RuntimeError`]. The names are part of the CLI contract
/// (`specs/001-plugin-runtime/contracts/cli.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Not a WebAssembly component, corrupted, or larger than the package limit.
    InvalidPackage,
    /// Does not implement plugin contract v0, or implements another version of it.
    ContractMismatch,
    /// Imports something the host does not offer.
    CapabilityRefused,
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
/// Only the runtime builds these, so every detail goes through the same constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    kind: ErrorKind,
    detail: String,
    refused_imports: Vec<String>,
}

impl RuntimeError {
    pub(crate) fn new(kind: ErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            refused_imports: Vec::new(),
        }
    }

    /// A `capability-refused` error that lists every import the host does not offer.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the import allowlist uses it from T032 on")
    )]
    pub(crate) fn capability_refused(imports: Vec<String>) -> Self {
        Self {
            kind: ErrorKind::CapabilityRefused,
            detail: imports.join(", "),
            refused_imports: imports,
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

    /// The imports a `capability-refused` error refused; empty for every other kind.
    pub fn refused_imports(&self) -> &[String] {
        &self.refused_imports
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
    fn capability_refused_lists_every_import() {
        let imports = vec![
            "wasi:filesystem/types@0.2.0".to_owned(),
            "wasi:sockets/tcp@0.2.0".to_owned(),
        ];
        let error = RuntimeError::capability_refused(imports.clone());
        assert_eq!(error.kind(), ErrorKind::CapabilityRefused);
        assert_eq!(error.refused_imports(), imports);
        assert_eq!(
            error.to_string(),
            "capability-refused: wasi:filesystem/types@0.2.0, wasi:sockets/tcp@0.2.0"
        );
    }
}
