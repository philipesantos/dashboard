//! The hostile-plugin suite, the security gate of Phase 1 (SC-001, Principle VIII).
//!
//! Each case must be refused or stopped with the error kind that names its rule
//! (`specs/001-plugin-runtime/data-model.md`, Hostile-plugin case), and a well-behaved plugin
//! must render normally right after it, on the same runtime (acceptance scenario 4).

mod common;

use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use common::fixture;
use runtime::{ErrorKind, Limits, Plugin, RenderReport, Runtime, RuntimeError};
use serde_json::json;

/// A render still running after this means a limit failed: the test fails instead of hanging.
const WATCHDOG: Duration = Duration::from_secs(30);

fn runtime() -> Runtime {
    Runtime::new(Limits::default()).expect("the runtime starts with the default limits")
}

fn load(runtime: &Runtime, name: &str) -> Arc<Plugin> {
    let plugin = runtime
        .load(&fixture(name))
        .unwrap_or_else(|error| panic!("{name} should load: {error}"));
    Arc::new(plugin)
}

fn load_error(runtime: &Runtime, name: &str) -> RuntimeError {
    match runtime.load(&fixture(name)) {
        Ok(_) => panic!("{name} was loaded, but it must be refused"),
        Err(error) => error,
    }
}

/// Renders on another thread, so a plugin that is never stopped fails the test.
fn render(plugin: &Arc<Plugin>, config: &str) -> RenderReport {
    let (sender, receiver) = mpsc::channel();
    let plugin = Arc::clone(plugin);
    let config = config.to_owned();
    thread::spawn(move || {
        let _ = sender.send(plugin.render(&config));
    });
    match receiver.recv_timeout(WATCHDOG) {
        Ok(report) => report,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("the plugin was not stopped within {WATCHDOG:?}")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("the render thread panicked"),
    }
}

fn render_error(report: &RenderReport) -> &RuntimeError {
    match &report.result {
        Ok(content) => panic!(
            "the plugin rendered {} bytes of content, but it must be stopped",
            content.to_string().len()
        ),
        Err(error) => error,
    }
}

/// Acceptance scenario 4: right after a hostile case, a well-behaved plugin renders normally.
fn assert_host_still_works(runtime: &Runtime) {
    let report = render(&load(runtime, "hello-widget"), "{}");
    assert_eq!(
        report
            .result
            .expect("hello-widget renders after a hostile case"),
        json!({"title": "Hello", "text": "Hello, world!"})
    );
}

#[test]
fn infinite_loop_runs_out_of_cpu_budget_and_keeps_its_log() {
    let runtime = runtime();
    let report = render(&load(&runtime, "infinite-loop"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::CpuBudgetExceeded);
    let messages: Vec<&str> = report.logs.iter().map(|log| log.message.as_str()).collect();
    assert_eq!(messages, ["looping"]);
    assert_host_still_works(&runtime);
}

#[test]
fn infinite_loop_with_unlimited_fuel_hits_the_time_limit_on_time() {
    let limits = Limits {
        fuel: u64::MAX / 2,
        time_limit: Duration::from_millis(500),
        ..Limits::default()
    };
    let runtime = Runtime::new(limits.clone()).unwrap();
    let plugin = load(&runtime, "infinite-loop");

    let start = Instant::now();
    let report = render(&plugin, "{}");
    let elapsed = start.elapsed();

    assert_eq!(render_error(&report).kind(), ErrorKind::TimeLimitExceeded);
    // SC-002: stopped no later than one second after the limit, and never before it
    assert!(
        elapsed >= limits.time_limit,
        "stopped after {elapsed:?}, before the limit"
    );
    assert!(
        elapsed <= limits.time_limit + Duration::from_secs(1),
        "stopped after {elapsed:?}, more than 1 s past the limit"
    );
    assert_host_still_works(&runtime);
}

#[test]
fn huge_allocation_hits_the_memory_cap() {
    let runtime = runtime();
    let report = render(&load(&runtime, "huge-alloc"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::MemoryLimitExceeded);
    assert_host_still_works(&runtime);
}

#[test]
fn deep_recursion_crashes_the_plugin_only() {
    let runtime = runtime();
    let report = render(&load(&runtime, "deep-recursion"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::PluginCrashed);
    assert_host_still_works(&runtime);
}

#[test]
fn reading_a_file_is_refused_at_load() {
    let runtime = runtime();
    let error = load_error(&runtime, "read-file");

    assert_eq!(error.kind(), ErrorKind::CapabilityRefused);
    assert!(
        error
            .refused_imports()
            .iter()
            .any(|import| import.starts_with("wasi:filesystem/")),
        "the refused imports should name the filesystem: {:?}",
        error.refused_imports()
    );
    assert_host_still_works(&runtime);
}

#[test]
fn opening_a_socket_is_refused_at_load() {
    let runtime = runtime();
    let error = load_error(&runtime, "open-socket");

    assert_eq!(error.kind(), ErrorKind::CapabilityRefused);
    assert!(
        error
            .refused_imports()
            .iter()
            .any(|import| import.starts_with("wasi:sockets/")),
        "the refused imports should name the sockets: {:?}",
        error.refused_imports()
    );
    assert_host_still_works(&runtime);
}

#[test]
fn malformed_output_is_rejected() {
    let runtime = runtime();
    let report = render(&load(&runtime, "malformed-output"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::InvalidOutput);
    assert_host_still_works(&runtime);
}

#[test]
fn oversized_output_is_rejected() {
    let runtime = runtime();
    let report = render(&load(&runtime, "oversized-output"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::OutputTooLarge);
    assert_host_still_works(&runtime);
}

#[test]
fn log_flood_keeps_100_messages_and_truncates_the_long_one() {
    let runtime = runtime();
    let report = render(&load(&runtime, "log-flood"), "{}");

    assert_eq!(
        report.result.as_ref().expect("log-flood renders"),
        &json!({"ok": true})
    );
    assert_eq!(report.logs.len(), 100);
    assert_eq!(report.dropped_logs, 900);
    let kept = report.logs[0]
        .message
        .strip_suffix(" [truncated]")
        .expect("the 4 KiB message is marked as truncated");
    // Cut at 1 KiB, moved back to the boundary of a 3-byte character
    assert!(
        kept.len() <= 1024 && kept.len() > 1024 - 3,
        "{} bytes kept",
        kept.len()
    );
    assert!(kept.chars().all(|c| c == '€'));
    assert_eq!(report.logs[1].message, "message 1");
    assert_host_still_works(&runtime);
}

#[test]
fn no_state_survives_between_calls_or_plugins() {
    let runtime = runtime();
    let plugin = load(&runtime, "state-leak");
    for _ in 0..2 {
        assert_eq!(render(&plugin, "{}").result.unwrap(), json!({"calls": 1}));
    }

    let first = load(&runtime, "state-leak");
    let second = load(&runtime, "state-leak");
    for plugin in [&first, &second, &first, &second] {
        assert_eq!(render(plugin, "{}").result.unwrap(), json!({"calls": 1}));
    }
    assert_host_still_works(&runtime);
}

#[test]
fn reading_outside_its_memory_crashes_the_plugin() {
    let runtime = runtime();
    let report = render(&load(&runtime, "out-of-bounds"), "{}");

    let error = render_error(&report);
    assert_eq!(error.kind(), ErrorKind::PluginCrashed);
    assert!(
        error.detail().contains("out of bounds memory access"),
        "the detail should name the trap: {}",
        error.detail()
    );
    assert_host_still_works(&runtime);
}

/// Characters that could make plugin text pass for the host's output: control characters and
/// bidirectional-text controls.
fn is_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        )
}

#[test]
fn plugin_text_cannot_imitate_the_host() {
    let runtime = runtime();
    let report = render(&load(&runtime, "log-spoof"), "{}");

    let error = render_error(&report);
    assert_eq!(error.kind(), ErrorKind::PluginError);
    let texts = report
        .logs
        .iter()
        .map(|log| log.message.as_str())
        .chain([error.detail()]);
    for text in texts {
        assert!(!text.chars().any(is_unsafe), "not escaped: {text:?}");
    }
    assert_eq!(
        report.logs[0].message,
        r"ok\n[runtime] 0 log messages dropped"
    );
    assert_host_still_works(&runtime);
}
