//! Host-defined resource limits (FR-005, FR-006). A plugin can neither read nor change them.

use std::fmt;
use std::time::Duration;

use wasmtime::ResourceLimiter;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

/// Limits applied to every plugin by the host.
///
/// User Story 4 measured the runtime on all three systems and confirmed these defaults
/// (`specs/001-plugin-runtime/performance.md`).
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

/// Instances, tables and memories one render call may create, and the elements of each table.
/// Far above what a plugin built from the contract uses; they bound what a crafted one could.
const MAX_INSTANCES: usize = 10;
const MAX_TABLES: usize = 10;
const MAX_MEMORIES: usize = 10;
const MAX_TABLE_ELEMENTS: usize = 10_000;

/// Enforces the memory cap of one render call, summed over every linear memory, so a component
/// with several memories cannot multiply it.
pub(crate) struct MemoryLimiter {
    max_memory_bytes: usize,
    /// Bytes granted so far in this call, across all memories. WebAssembly memories never
    /// shrink, so this only grows until the call ends.
    granted: usize,
}

impl MemoryLimiter {
    pub(crate) fn new(max_memory_bytes: usize) -> Self {
        Self {
            max_memory_bytes,
            granted: 0,
        }
    }
}

impl ResourceLimiter for MemoryLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let total = self.granted.saturating_add(desired.saturating_sub(current));
        if total > self.max_memory_bytes {
            // An error instead of `Ok(false)`: the call stops right away with an error the
            // runtime reports as the memory limit, instead of the plugin seeing a failed growth.
            return Err(MemoryLimitExceeded {
                requested: total,
                limit: self.max_memory_bytes,
            }
            .into());
        }
        self.granted = total;
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= MAX_TABLE_ELEMENTS)
    }

    fn instances(&self) -> usize {
        MAX_INSTANCES
    }

    fn tables(&self) -> usize {
        MAX_TABLES
    }

    fn memories(&self) -> usize {
        MAX_MEMORIES
    }
}

/// The error a render call stops with when its memories would grow beyond the cap.
#[derive(Debug)]
pub(crate) struct MemoryLimitExceeded {
    requested: usize,
    limit: usize,
}

impl fmt::Display for MemoryLimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tried to grow its memory to {} bytes, over the cap of {}",
            self.requested, self.limit
        )
    }
}

impl std::error::Error for MemoryLimitExceeded {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grants_growth_up_to_the_cap() {
        let mut limiter = MemoryLimiter::new(32 * MIB);
        assert!(limiter.memory_growing(0, 16 * MIB, None).unwrap());
        assert!(limiter.memory_growing(16 * MIB, 32 * MIB, None).unwrap());
        assert!(
            limiter
                .memory_growing(32 * MIB, 32 * MIB + 1, None)
                .is_err()
        );
    }

    #[test]
    fn sums_every_memory_of_the_call() {
        let mut limiter = MemoryLimiter::new(Limits::default().max_memory_bytes);
        assert!(limiter.memory_growing(0, 20 * MIB, None).unwrap());
        let error = limiter.memory_growing(0, 20 * MIB, None).unwrap_err();
        assert!(error.downcast_ref::<MemoryLimitExceeded>().is_some());
    }

    #[test]
    fn caps_table_elements() {
        let mut limiter = MemoryLimiter::new(32 * MIB);
        assert!(limiter.table_growing(0, MAX_TABLE_ELEMENTS, None).unwrap());
        assert!(
            !limiter
                .table_growing(0, MAX_TABLE_ELEMENTS + 1, None)
                .unwrap()
        );
    }
}
