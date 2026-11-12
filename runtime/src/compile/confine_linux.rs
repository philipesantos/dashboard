//! The compile process's confinement on Linux (`specs/010-plugin-installation/research.md`, R13):
//! before it reads the package, the child takes away from itself every way to reach anything but
//! its own memory and its three standard streams: Landlock before it starts its compile threads,
//! so they inherit it, and seccomp once they exist, on all of them.
//!
//! - Landlock, where the kernel has it: a ruleset that handles every file-system right and grants
//!   none, so no file can be opened, created or changed.
//! - seccomp, on every thread of the process at once (`SECCOMP_FILTER_FLAG_TSYNC`): a filter that
//!   allows only the system calls compiling needs (memory, futexes, reading standard input,
//!   writing standard output and error, signals to itself, and exiting), and kills the process on
//!   any other, such as `socket`, `connect`, `execve`, `ptrace` or `clone`; and a second filter,
//!   stacked on it, that answers opening a file with `EACCES`. glibc may open a file of its own
//!   from a compile thread at times (found by the tests, under resource limits, only some runs),
//!   and handles the refusal; no file opens either way. Of two filters, the stricter answer wins.
//!
//! The list was taken from the calls a compile makes once the package is read (strace, Wasmtime
//! 50, glibc 2.43, kernel 7.0), with the thread pool already built. `no_new_privs` is set by both.
//!
//! Part of the security core (constitution v2.7.0, Principle IX).

use std::collections::BTreeMap;

use landlock::{ABI, Access, AccessFs, Ruleset, RulesetAttr};
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule, TargetArch,
};

/// The first part, before the process starts its compile threads: Landlock, which binds the
/// calling thread and the threads it starts afterwards, not those that already exist. Best effort:
/// a kernel without it still gets the filters.
pub(super) fn before_threads() -> Result<(), String> {
    landlock()
}

/// The second part, once the threads exist: the seccomp filters, applied to every thread at
/// once. Not best effort: if they cannot be applied, the compile is refused.
pub(super) fn after_threads() -> Result<(), String> {
    seccomp()
}

fn landlock() -> Result<(), String> {
    Ruleset::default()
        .handle_access(AccessFs::from_all(ABI::V6))
        .and_then(|ruleset| ruleset.create())
        .and_then(|ruleset| ruleset.restrict_self())
        .map(|_| ())
        .map_err(|error| format!("Landlock: {error}"))
}

/// A rule matching when argument `index` equals `value`.
fn equals(index: u8, value: u64) -> Result<SeccompRule, String> {
    SeccompCondition::new(index, SeccompCmpArgLen::Qword, SeccompCmpOp::Eq, value)
        .and_then(|condition| SeccompRule::new(vec![condition]))
        .map_err(|error| format!("seccomp: {error}"))
}

fn seccomp() -> Result<(), String> {
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    // Allowed whatever their arguments: memory, waiting, the thread's own bookkeeping, signals'
    // own handling, the clock, randomness, and the end
    for call in [
        libc::SYS_mmap,
        libc::SYS_munmap,
        libc::SYS_mremap,
        libc::SYS_mprotect,
        libc::SYS_madvise,
        libc::SYS_brk,
        libc::SYS_futex,
        libc::SYS_sched_yield,
        libc::SYS_sched_getaffinity,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigreturn,
        libc::SYS_sigaltstack,
        libc::SYS_gettid,
        libc::SYS_getpid,
        libc::SYS_getrandom,
        libc::SYS_clock_gettime,
        libc::SYS_exit,
        libc::SYS_exit_group,
    ] {
        rules.insert(call, Vec::new());
    }
    // Standard input only, to read the package
    rules.insert(libc::SYS_read, vec![equals(0, 0)?]);
    // Standard output and error only, to write the compiled form and a refusal's reason
    rules.insert(libc::SYS_write, vec![equals(0, 1)?, equals(0, 2)?]);
    // A signal to this process only, as `abort` sends when an allocation fails
    rules.insert(
        libc::SYS_tgkill,
        vec![equals(0, u64::from(std::process::id()))?],
    );
    // Opening a file is let through this filter, and refused by the next one
    let opens = opens();
    for call in &opens {
        rules.insert(*call, Vec::new());
    }
    let arch = TargetArch::try_from(std::env::consts::ARCH)
        .map_err(|error| format!("seccomp: {error}"))?;
    let allowed = SeccompFilter::new(
        rules,
        SeccompAction::KillProcess,
        SeccompAction::Allow,
        arch,
    )
    .map_err(|error| format!("seccomp: {error}"))?;
    let refused = SeccompFilter::new(
        opens.into_iter().map(|call| (call, Vec::new())).collect(),
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EACCES as u32),
        arch,
    )
    .map_err(|error| format!("seccomp: {error}"))?;
    // The refusal first: installing a filter takes `prctl` and `seccomp`, which the other one,
    // once in place, kills
    for filter in [refused, allowed] {
        let program: BpfProgram = filter
            .try_into()
            .map_err(|error| format!("seccomp: {error}"))?;
        seccompiler::apply_filter_all_threads(&program)
            .map_err(|error| format!("seccomp: {error}"))?;
    }
    Ok(())
}

/// The calls that open a file on this architecture.
fn opens() -> Vec<i64> {
    #[cfg(target_arch = "x86_64")]
    return vec![libc::SYS_open, libc::SYS_openat, libc::SYS_openat2];
    #[cfg(not(target_arch = "x86_64"))]
    vec![libc::SYS_openat, libc::SYS_openat2]
}
