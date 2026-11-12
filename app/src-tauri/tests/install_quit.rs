//! `quit-mid-compile` (specs/010-plugin-installation/, FR-023; task T020), in a test binary of
//! its own: it looks for this process's compile children, which the tests of another file,
//! running beside it as threads of one process, would also start.

mod common;

use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use common::{App, BASIC, package_named};
use dashboard_lib::install::Started;

/// The children of this process still running with `--compile-package`, read from `/proc`.
fn compile_children() -> Vec<u32> {
    let me = std::process::id();
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let stat = std::fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("stat"))
            .unwrap_or_default();
        // The parent is the fourth field, after the name in parentheses
        let parent = stat
            .rsplit_once(") ")
            .and_then(|(_, rest)| rest.split(' ').nth(1))
            .and_then(|field| field.parse::<u32>().ok());
        let cmdline = std::fs::read(Path::new("/proc").join(pid.to_string()).join("cmdline"))
            .unwrap_or_default();
        if parent == Some(me) && String::from_utf8_lossy(&cmdline).contains("--compile-package") {
            found.push(pid);
        }
    }
    found
}

/// `quit-mid-compile`: stopping the host while a request compiles kills the child and keeps
/// nothing. It looks for the child in `/proc`; where there is none, it says so and checks nothing
/// (the per-system process checks come with the compile limits, task T037).
#[test]
fn quitting_while_a_package_compiles_kills_the_compile_process() {
    if !Path::new("/proc/self/stat").exists() {
        eprintln!("no /proc on this system: the compile process is not looked for");
        return;
    }
    let app = App::new();
    app.source.publish(
        "dashboard-fixtures/compile-heavy",
        9,
        "v0.1.0",
        package_named("compile-heavy"),
    );
    let widgets = Arc::new(app.start());
    let running = Arc::clone(&widgets);
    let started =
        thread::spawn(move || running.install_start("github.com/dashboard-fixtures/compile-heavy"));
    let deadline = Instant::now() + Duration::from_secs(20);
    while compile_children().is_empty() {
        assert!(
            Instant::now() < deadline,
            "the compile process never started"
        );
        thread::sleep(Duration::from_millis(5));
    }
    widgets.stop_requests();
    let outcome = started.join().unwrap();
    assert!(
        matches!(
            &outcome,
            Started::Refused {
                refused: "unknown-request"
            }
        ) || matches!(&outcome, Started::Error { error } if error.kind == "compile-failed"),
        "{outcome:?}"
    );
    assert!(compile_children().is_empty(), "a compile process was left");
    app.assert_nothing_kept();
    // And the next request can start
    assert!(matches!(
        widgets.install_start(BASIC),
        Started::Request { .. }
    ));
}
