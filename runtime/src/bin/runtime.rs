//! `runtime`: the command-line test tool (`specs/001-plugin-runtime/contracts/cli.md`).
//!
//! `runtime run <plugin> [--config <json> | --config-file <path>]` loads a plugin, calls its
//! `render` once and prints the widget content, or the error that refused or stopped it.

use std::fs::File;
use std::io::Read;
use std::process::ExitCode;

use runtime::{Limits, Runtime};

const USAGE: &str = "usage: runtime run <plugin> [--config <json> | --config-file <path>]";

/// Exit status when the plugin was refused, failed or was stopped.
const PLUGIN_FAILED: u8 = 1;
/// Exit status for usage errors: unknown command, missing file, invalid configuration.
const USAGE_ERROR: u8 = 2;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => match parse_run(&args[1..]) {
            Ok((plugin, config)) => run(&plugin, &config),
            Err(message) => usage_error(&message),
        },
        Some(command) => usage_error(&format!("unknown command `{command}`")),
        None => usage_error("no command given"),
    }
}

/// Returns the plugin path and the configuration, already checked to be valid JSON.
fn parse_run(args: &[String]) -> Result<(String, String), String> {
    let mut plugin = None;
    let mut config = None;
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
    Ok((plugin, config))
}

fn run(plugin_path: &str, config: &str) -> ExitCode {
    let limits = Limits::default();
    let bytes = match read_plugin(plugin_path, limits.max_package_bytes) {
        Ok(bytes) => bytes,
        Err(message) => return usage_error(&message),
    };
    let runtime = match Runtime::new(limits) {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("error: cannot start the runtime: {error:#}");
            return ExitCode::from(PLUGIN_FAILED);
        }
    };
    let plugin = match runtime.load(&bytes) {
        Ok(plugin) => plugin,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(PLUGIN_FAILED);
        }
    };

    let report = plugin.render(config);
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
