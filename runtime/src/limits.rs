//! Host-defined resource limits (FR-005, FR-006). A plugin can neither read nor change them.

use std::time::Duration;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

/// Limits applied to every plugin by the host.
///
/// The defaults are provisional until User Story 4 measures the runtime on all three systems
/// (`specs/001-plugin-runtime/research.md`, R9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Larger packages are refused as `invalid-package`.
    pub max_package_bytes: usize,
    /// CPU budget per render call, in Wasmtime fuel units.
    pub fuel: u64,
    /// Wall-clock limit per render call.
    pub time_limit: Duration,
    /// Sum of all linear memories per render call.
    pub max_memory_bytes: usize,
    /// Size cap of the widget content, checked before parsing it.
    pub max_output_bytes: usize,
    /// Log messages kept per render call; the rest are dropped and counted.
    pub max_log_messages: usize,
    /// Longer log messages are truncated.
    pub max_log_message_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_package_bytes: 10 * MIB,
            fuel: 500_000_000,
            time_limit: Duration::from_secs(1),
            max_memory_bytes: 32 * MIB,
            max_output_bytes: 64 * KIB,
            max_log_messages: 100,
            max_log_message_bytes: KIB,
        }
    }
}
