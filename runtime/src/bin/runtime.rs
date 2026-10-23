//! `runtime`: the command-line test tool (`specs/001-plugin-runtime/contracts/cli.md`,
//! `specs/002-plugin-permissions/contracts/cli.md`).
//!
//! `runtime run <plugin>` loads a plugin, calls its `render` once and prints the widget content,
//! or the error that refused or stopped it. `runtime bench <plugin>` measures how long loading
//! and rendering take.
//!
//! The tool is the host's control in this phase: it works in development mode only, on local
//! package files, with the development grant record (`--grants <path>`, or the default one in
//! the system's data directory). A package's origin is its file's canonical path, and an
//! approval applies only to the file it was given for: another file that copies an approved
//! plugin's id inherits none of its grants (FR-020).

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use runtime::grants::GrantStore;
use runtime::{Limits, Plugin, Runtime};

const USAGE: &str = "usage: runtime run <plugin> [--config <json> | --config-file <path>] [--grants <path>]
       runtime bench <plugin> [--iterations <n>] [--config <json> | --config-file <path>] [--grants <path>]";

/// Exit status when the plugin was refused, failed or was stopped.
const PLUGIN_FAILED: u8 = 1;
/// Exit status for usage errors: unknown command, missing file, invalid configuration.
const USAGE_ERROR: u8 = 2;

/// Render calls `bench` measures when `--iterations` is not given.
const DEFAULT_ITERATIONS: u32 = 1000;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, options) = match args.first().map(String::as_str) {
        Some(command @ ("run" | "bench")) => (command, parse(&args[1..], command == "bench")),
        Some(command) => return usage_error(&format!("unknown command `{command}`")),
        None => return usage_error("no command given"),
    };
    match (command, options) {
        (_, Err(message)) => usage_error(&message),
        ("run", Ok(options)) => run(&options),
        (_, Ok(options)) => bench(&options),
    }
}

struct Options {
    plugin: String,
    /// Already checked to be valid JSON.
    config: String,
    /// Only for `bench`.
    iterations: u32,
    /// The grant record given with `--grants`.
    grants: Option<PathBuf>,
}

fn parse(args: &[String], bench: bool) -> Result<Options, String> {
    let mut plugin = None;
    let mut config = None;
    let mut iterations = DEFAULT_ITERATIONS;
    let mut grants = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            option @ ("--config" | "--config-file") => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{option} needs a value"))?;
                if config.is_some() {
                    return Err("give the configuration only once".to_owned());
                }
                config = Some(if option == "--config" {
                    value.clone()
                } else {
                    std::fs::read_to_string(value).map_err(|error| {
                        format!("cannot read configuration file {value}: {error}")
                    })?
                });
            }
            "--grants" => {
                let value = args.next().ok_or("--grants needs a path")?;
                if grants.replace(PathBuf::from(value)).is_some() {
                    return Err("give --grants only once".to_owned());
                }
            }
            "--iterations" if bench => {
                iterations = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .filter(|&n| n > 0)
                    .ok_or("--iterations needs a whole number above 0")?;
            }
            option if option.starts_with("--") => {
                return Err(format!("unknown option `{option}`"));
            }
            path => {
                if plugin.is_some() {
                    return Err(format!("unexpected argument `{path}`"));
                }
                plugin = Some(path.to_owned());
            }
        }
    }
    let plugin = plugin.ok_or("no plugin file given")?;
    let config = config.unwrap_or_else(|| "{}".to_owned());
    serde_json::from_str::<serde_json::Value>(&config)
        .map_err(|error| format!("the configuration is not valid JSON: {error}"))?;
    Ok(Options {
        plugin,
        config,
        iterations,
        grants,
    })
}

fn run(options: &Options) -> ExitCode {
    let plugin = match load(options) {
        Ok((plugin, _)) => plugin,
        Err(status) => return status,
    };

    let report = plugin.render(&options.config);
    for log in &report.logs {
        eprintln!("[plugin] {}: {}", log.level.as_str(), log.message);
    }
    if report.dropped_logs > 0 {
        eprintln!("[runtime] {} log messages dropped", report.dropped_logs);
    }
    match report.result {
        Ok(content) => {
            let pretty =
                serde_json::to_string_pretty(&content).expect("a JSON value always serializes");
            println!("{pretty}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(PLUGIN_FAILED)
        }
    }
}

/// Times one load, then `iterations` render calls, each with its own instantiation.
fn bench(options: &Options) -> ExitCode {
    let (plugin, load_time) = match load(options) {
        Ok(loaded) => loaded,
        Err(status) => return status,
    };

    let mut times = Vec::with_capacity(options.iterations as usize);
    for _ in 0..options.iterations {
        let start = Instant::now();
        let report = plugin.render(&options.config);
        times.push(start.elapsed());
        if let Err(error) = report.result {
            eprintln!("error: {error}");
            return ExitCode::from(PLUGIN_FAILED);
        }
    }
    times.sort();

    println!("load_ms: {:.1}", load_time.as_secs_f64() * 1000.0);
    println!("render_p50_us: {}", percentile(&times, 50).as_micros());
    println!("render_p95_us: {}", percentile(&times, 95).as_micros());
    println!("render_max_us: {}", percentile(&times, 100).as_micros());
    ExitCode::SUCCESS
}

/// The nearest-rank percentile of sorted, non-empty times.
fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    let rank = (percent * sorted.len()).div_ceil(100).max(1);
    sorted[rank - 1]
}

/// Reads and loads a plugin with its grants, timing the load. On failure, prints why and
/// returns the exit status.
fn load(options: &Options) -> Result<(Plugin, Duration), ExitCode> {
    let limits = Limits::default();
    let path = &options.plugin;
    let bytes =
        read_plugin(path, limits.max_package_bytes).map_err(|message| usage_error(&message))?;
    let origin = origin(path).map_err(|message| usage_error(&message))?;
    let grants = grant_store(options.grants.as_ref()).map_err(|message| usage_error(&message))?;
    let runtime = Runtime::new(limits).map_err(|error| {
        eprintln!("error: cannot start the runtime: {error:#}");
        ExitCode::from(PLUGIN_FAILED)
    })?;
    let start = Instant::now();
    let plugin = runtime
        .load(&bytes, &origin, Arc::new(grants))
        .map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::from(PLUGIN_FAILED)
        })?;
    Ok((plugin, start.elapsed()))
}

/// The package's origin: its file's canonical path, so the same file has one origin however it
/// is named on the command line.
fn origin(path: &str) -> Result<String, String> {
    std::fs::canonicalize(path)
        .map_err(|error| format!("cannot read plugin file {path}: {error}"))?
        .into_os_string()
        .into_string()
        .map_err(|_| format!("the path of plugin file {path} is not UTF-8"))
}

/// The record given with `--grants`, or the default development record.
fn grant_store(path: Option<&PathBuf>) -> Result<GrantStore, String> {
    match path {
        Some(path) => Ok(GrantStore::open(path)),
        None => GrantStore::default_development_path()
            .map(GrantStore::open)
            .ok_or_else(|| {
                "this system has no data directory for the grant record; give --grants <path>"
                    .to_owned()
            }),
    }
}

/// Reads at most `max_bytes + 1` bytes, so a huge or endless file (for example `/dev/zero`) is
/// never read whole; `Runtime::load` decides whether the size is acceptable.
fn read_plugin(path: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let file =
        File::open(path).map_err(|error| format!("cannot read plugin file {path}: {error}"))?;
    let mut bytes = Vec::new();
    file.take((max_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read plugin file {path}: {error}"))?;
    Ok(bytes)
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("error: {message}\n{USAGE}");
    ExitCode::from(USAGE_ERROR)
}
