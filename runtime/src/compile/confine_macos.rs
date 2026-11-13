//! The compile process's confinement and memory limit on macOS
//! (`specs/010-plugin-installation/research.md`, R2, R13).
//!
//! - `sandbox_init` with the system's pure-computation profile: no file, no network, no Mach
//!   service (the keychain's included); the descriptors the process already holds, its three
//!   standard streams, keep working.
//! - macOS enforces no memory rlimit, so a watchdog thread, started before the sandbox, reads the
//!   process's peak resident memory every 50 ms (`getrusage`, which the sandbox allows) and ends
//!   the process above the limit the parent passed, saying so on standard error as a failed
//!   allocation would. It also ends the process when its parent is gone, as Linux's parent-death
//!   signal does.
//!
//! Part of the security core (constitution v2.7.0, Principle IX). Checked on macOS by CI only.

use std::ffi::{CStr, c_char, c_int};
use std::time::Duration;

/// `SANDBOX_NAMED`: the profile is one of the system's, by name.
const SANDBOX_NAMED: u64 = 0x0001;

/// The exit code the watchdog ends the process with, which the parent reads as the memory limit.
const MEMORY_EXIT: c_int = super::EXIT_MEMORY as c_int;

unsafe extern "C" {
    fn sandbox_init(profile: *const c_char, flags: u64, errorbuf: *mut *mut c_char) -> c_int;
    fn sandbox_free_error(errorbuf: *mut c_char);
}

/// Starts the watchdog for `memory_bytes`, then confines the process.
pub(super) fn confine(memory_bytes: Option<u64>) -> Result<(), String> {
    watchdog(memory_bytes)?;
    let mut error: *mut c_char = std::ptr::null_mut();
    // SAFETY: a constant, nul-terminated profile name; on failure the error buffer is the
    // system's, read once and freed with its own function
    let status = unsafe { sandbox_init(c"pure-computation".as_ptr(), SANDBOX_NAMED, &mut error) };
    if status == 0 {
        return Ok(());
    }
    let reason = if error.is_null() {
        "the system refused".to_owned()
    } else {
        // SAFETY: a nul-terminated string the system wrote
        let text = unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned();
        unsafe { sandbox_free_error(error) };
        text
    };
    Err(format!("sandbox_init: {reason}"))
}

fn watchdog(memory_bytes: Option<u64>) -> Result<(), String> {
    // SAFETY: getppid has no arguments and cannot fail
    let parent = unsafe { libc::getppid() };
    std::thread::Builder::new()
        .name("compile-watchdog".to_owned())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(50));
                // SAFETY: as above
                if unsafe { libc::getppid() } != parent {
                    // SAFETY: ends this process at once, as the parent is gone
                    unsafe { libc::_exit(1) };
                }
                if let Some(limit) = memory_bytes
                    && peak_resident_bytes() > limit
                {
                    let message =
                        b"could not allocate: the compile process passed its memory limit\n";
                    // SAFETY: a write of a constant buffer to standard error, then the end
                    unsafe {
                        libc::write(2, message.as_ptr().cast(), message.len());
                        libc::_exit(MEMORY_EXIT);
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|error| format!("the memory watchdog: {error}"))
}

/// The process's peak resident memory, in bytes (macOS gives `ru_maxrss` in bytes).
fn peak_resident_bytes() -> u64 {
    // SAFETY: getrusage fills the zeroed struct it is given
    let usage = unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        usage
    };
    u64::try_from(usage.ru_maxrss).unwrap_or(0)
}
