//! Host-defined resource limits (FR-005, FR-006 of feature 001; FR-008, FR-010 of feature 003;
//! FR-005, FR-007 of feature 004; FR-009 to FR-011 of feature 005).
//! A plugin can neither read nor change them.

use std::fmt;
use std::time::Duration;

use wasmtime::ResourceLimiter;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

/// Limits applied to every plugin by the host.
///
/// User Story 4 of feature 001 measured the runtime on all three systems and confirmed the render
/// defaults (`specs/001-plugin-runtime/performance.md`). The update and network limits are
/// provisional values of `specs/003-plugin-connectors/research.md` (R10), measured before the
/// feature closes. An update runs with the same fuel and memory caps as a render. The storage
/// limits are provisional values of `specs/004-plugin-storage/research.md` (R10), and the
/// credential limits those of `specs/005-plugin-credentials/research.md` (R13), measured the same
/// way.
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
    /// Wall-clock limit per update call, larger than render's because an update waits on the
    /// network; its CPU and memory caps are render's.
    pub update_time_limit: Duration,
    /// Wall-clock limit of one connector call (resolve, connect, TLS, response), cut to what is
    /// left of the update.
    pub call_time_limit: Duration,
    /// Connector calls per run of `update` or `render`, every call counted, refused ones
    /// included; past it, calls are refused and the request log gets one summary entry.
    pub max_calls_per_run: usize,
    /// Arguments per call.
    pub max_arguments: usize,
    /// Longest argument name, in characters.
    pub max_argument_name_chars: usize,
    /// Longest request address (path and query), in bytes.
    pub max_request_address_bytes: usize,
    /// Largest request body.
    pub max_request_body_bytes: usize,
    /// Largest response body of one call; the widget never receives part of a larger one.
    pub max_response_body_bytes: usize,
    /// Response bodies of one update together.
    pub max_update_response_bytes: usize,
    /// Response headers of one call together.
    pub max_response_header_bytes: usize,
    /// Redirects the host follows in one call.
    pub max_redirects: usize,
    /// Size cap of the data document an update returns, checked before parsing it.
    pub max_update_output_bytes: usize,
    /// Calls per minute of one widget.
    pub calls_per_minute_per_widget: u32,
    /// Calls per minute to one destination host, across every widget of one runtime.
    pub calls_per_minute_per_host: u32,
    /// Size of the request log before it is rotated, keeping one previous file.
    pub max_request_log_bytes: usize,
    /// Longest argument value written to the request log, in characters, before ` [truncated]`.
    pub max_logged_value_chars: usize,
    /// Longest connector or operation name written to the request log, in characters.
    pub max_logged_name_chars: usize,
    /// The host cap: the largest storage quota a manifest may declare, in usage bytes. A larger
    /// one is `invalid-manifest`.
    pub max_storage_quota_bytes: u64,
    /// What each stored entry counts in usage on top of its name and value, the same on every
    /// system.
    pub storage_entry_cost_bytes: u64,
    /// Longest storage name, in bytes of UTF-8; also the longest `prefix` and `after` of `list`.
    pub max_storage_name_bytes: usize,
    /// Largest stored value.
    pub max_storage_value_bytes: usize,
    /// Storage calls per run of `update` or `render`, every call counted, refused ones
    /// included; past it, calls are refused.
    pub max_storage_calls_per_run: usize,
    /// Names and values of every `set` of one update together, replaced ones included.
    pub max_storage_written_bytes_per_update: u64,
    /// Names in one page of `list`, fixed by the host.
    pub storage_list_page: usize,
    /// The store's file may reach this many times the approved quota, plus
    /// `storage_file_cap_extra_bytes`, rounded up to whole pages and never below the file's
    /// current size. Usage is logical, and a file can reach several times its usage.
    pub storage_file_cap_factor: u64,
    /// Added to the file cap, for the file's own pages.
    pub storage_file_cap_extra_bytes: u64,
    /// The longest a storage call waits for another process writing the store, cut to what is
    /// left of the run's time limit.
    pub max_storage_busy_wait: Duration,
    /// The longest a sign-in waits for the browser's callback.
    pub sign_in_time_limit: Duration,
    /// Largest request head the sign-in's loopback listener reads.
    pub max_callback_head_bytes: usize,
    /// The longest the listener waits for a request head once a connection is open.
    pub callback_read_time: Duration,
    /// Largest answer of a token, refresh, revocation or identity request.
    pub max_sign_in_response_bytes: usize,
    /// Largest document the vault stores for one account (Windows' credential blob limit).
    pub max_secret_bytes: usize,
    /// Longest key a maintainer may type, in bytes.
    pub max_key_bytes: usize,
    /// Longest account name or id shown, in characters, before it is cut.
    pub max_account_name_chars: usize,
    /// The longest the development tool waits for the keychain, time for the person to unlock
    /// it; a widget's call waits at most its own time limit.
    pub vault_wait_tool: Duration,
    /// A token that expires within this is refreshed before it is sent.
    pub refresh_margin: Duration,
    /// Refreshes one call may cause.
    pub refreshes_per_call: usize,
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
            update_time_limit: Duration::from_secs(10),
            call_time_limit: Duration::from_secs(5),
            max_calls_per_run: 20,
            max_arguments: 16,
            max_argument_name_chars: 32,
            max_request_address_bytes: 8 * KIB,
            max_request_body_bytes: 64 * KIB,
            max_response_body_bytes: MIB,
            max_update_response_bytes: 4 * MIB,
            max_response_header_bytes: 16 * KIB,
            max_redirects: 5,
            max_update_output_bytes: 256 * KIB,
            calls_per_minute_per_widget: 30,
            calls_per_minute_per_host: 60,
            max_request_log_bytes: 2 * MIB,
            max_logged_value_chars: 256,
            max_logged_name_chars: 160,
            max_storage_quota_bytes: 8 * MIB as u64,
            storage_entry_cost_bytes: 64,
            max_storage_name_bytes: 256,
            max_storage_value_bytes: 256 * KIB,
            max_storage_calls_per_run: 1000,
            max_storage_written_bytes_per_update: MIB as u64,
            storage_list_page: 100,
            storage_file_cap_factor: 5,
            storage_file_cap_extra_bytes: 64 * KIB as u64,
            max_storage_busy_wait: Duration::from_secs(2),
            sign_in_time_limit: Duration::from_secs(10 * 60),
            max_callback_head_bytes: 8 * KIB,
            callback_read_time: Duration::from_secs(2),
            max_sign_in_response_bytes: 64 * KIB,
            max_secret_bytes: 2560,
            max_key_bytes: 1024,
            max_account_name_chars: 100,
            vault_wait_tool: Duration::from_secs(2 * 60),
            refresh_margin: Duration::from_secs(60),
            refreshes_per_call: 1,
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

    /// The defaults of research R10 (`specs/003-plugin-connectors/research.md`).
    #[test]
    fn update_and_network_defaults_are_those_of_the_plan() {
        let limits = Limits::default();
        assert_eq!(limits.update_time_limit, Duration::from_secs(10));
        assert!(limits.update_time_limit > limits.time_limit);
        assert_eq!(limits.call_time_limit, Duration::from_secs(5));
        assert_eq!(limits.max_calls_per_run, 20);
        assert_eq!(limits.max_arguments, 16);
        assert_eq!(limits.max_argument_name_chars, 32);
        assert_eq!(limits.max_request_address_bytes, 8 * KIB);
        assert_eq!(limits.max_request_body_bytes, 64 * KIB);
        assert_eq!(limits.max_response_body_bytes, MIB);
        assert_eq!(limits.max_update_response_bytes, 4 * MIB);
        assert_eq!(limits.max_response_header_bytes, 16 * KIB);
        assert_eq!(limits.max_redirects, 5);
        assert_eq!(limits.max_update_output_bytes, 256 * KIB);
        assert_eq!(limits.calls_per_minute_per_widget, 30);
        assert_eq!(limits.calls_per_minute_per_host, 60);
        assert_eq!(limits.max_request_log_bytes, 2 * MIB);
        assert_eq!(limits.max_logged_value_chars, 256);
        assert_eq!(limits.max_logged_name_chars, 160);
    }

    /// The defaults of research R10 (`specs/004-plugin-storage/research.md`).
    #[test]
    fn storage_defaults_are_those_of_the_plan() {
        let limits = Limits::default();
        assert_eq!(limits.max_storage_quota_bytes, 8 * MIB as u64);
        assert_eq!(limits.storage_entry_cost_bytes, 64);
        assert_eq!(limits.max_storage_name_bytes, 256);
        assert_eq!(limits.max_storage_value_bytes, 256 * KIB);
        assert_eq!(limits.max_storage_calls_per_run, 1000);
        assert_eq!(limits.max_storage_written_bytes_per_update, MIB as u64);
        assert_eq!(limits.storage_list_page, 100);
        assert_eq!(limits.storage_file_cap_factor, 5);
        assert_eq!(limits.storage_file_cap_extra_bytes, 64 * KIB as u64);
        assert_eq!(limits.max_storage_busy_wait, Duration::from_secs(2));
        // The smallest quota holds one empty entry
        assert!(limits.storage_entry_cost_bytes <= limits.max_storage_quota_bytes);
    }

    /// The defaults of research R13 (`specs/005-plugin-credentials/research.md`).
    #[test]
    fn credential_defaults_are_those_of_the_plan() {
        let limits = Limits::default();
        assert_eq!(limits.sign_in_time_limit, Duration::from_secs(600));
        assert_eq!(limits.max_callback_head_bytes, 8 * KIB);
        assert_eq!(limits.callback_read_time, Duration::from_secs(2));
        assert_eq!(limits.max_sign_in_response_bytes, 64 * KIB);
        assert_eq!(limits.max_secret_bytes, 2560);
        assert_eq!(limits.max_key_bytes, 1024);
        assert_eq!(limits.max_account_name_chars, 100);
        assert_eq!(limits.vault_wait_tool, Duration::from_secs(120));
        assert_eq!(limits.refresh_margin, Duration::from_secs(60));
        assert_eq!(limits.refreshes_per_call, 1);
        // A key, with its JSON around it, fits in what the vault stores
        assert!(limits.max_key_bytes + 16 <= limits.max_secret_bytes);
    }

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
