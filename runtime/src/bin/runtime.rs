//! `runtime`: the command-line test tool (`specs/001-plugin-runtime/contracts/cli.md`,
//! `specs/002-plugin-permissions/contracts/cli.md`).
//!
//! `runtime run <plugin>` loads a plugin, calls its `render` once and prints the widget content,
//! or the error that refused or stopped it. `runtime bench <plugin>` measures how long loading
//! and rendering take. `runtime permissions <plugin>` shows what a plugin requests and what is
//! granted, and `runtime approve <plugin>` grants its required permissions, all together.
//! `runtime optional <plugin> <permission> on|off` turns one optional permission on or off, and
//! `runtime remove <plugin-id>` removes a plugin's approval and every grant with it.
//! `runtime pack <component> --manifest <file> --output <file>` checks a manifest and adds it to
//! a component, for authors outside this repository's xtask.
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

use runtime::grants::{GrantStore, Status};
use runtime::manifest::MAX_MANIFEST_BYTES;
use runtime::manifest::PluginId;
use runtime::{CheckedPackage, Limits, Plugin, Runtime, escape, package};

const USAGE: &str = "usage: runtime run <plugin> [--config <json> | --config-file <path>] [--grants <path>]
       runtime bench <plugin> [--iterations <n>] [--config <json> | --config-file <path>] [--grants <path>]
       runtime permissions <plugin> [--grants <path>]
       runtime approve <plugin> [--grants <path>]
       runtime optional <plugin> <permission> on|off [--grants <path>]
       runtime remove <plugin-id> [--grants <path>]
       runtime pack <component> --manifest <file> --output <file>";

/// Exit status when the plugin or the change was refused, or the plugin failed or was stopped.
const PLUGIN_FAILED: u8 = 1;
/// Exit status for usage errors: unknown command, missing file, invalid configuration.
const USAGE_ERROR: u8 = 2;

/// Render calls `bench` measures when `--iterations` is not given.
const DEFAULT_ITERATIONS: u32 = 1000;

/// What `permissions` says of login providers until accounts can be connected.
const LOGIN_PROVIDERS_LATER: &str = "(connecting accounts arrives in feature 005)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Run,
    Bench,
    Permissions,
    Approve,
    Optional,
    Remove,
    Pack,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match args.first().map(String::as_str) {
        Some("run") => Command::Run,
        Some("bench") => Command::Bench,
        Some("permissions") => Command::Permissions,
        Some("approve") => Command::Approve,
        Some("optional") => Command::Optional,
        Some("remove") => Command::Remove,
        Some("pack") => Command::Pack,
        Some(command) => return usage_error(&format!("unknown command `{command}`")),
        None => return usage_error("no command given"),
    };
    let options = match parse(&args[1..], command) {
        Ok(options) => options,
        Err(message) => return usage_error(&message),
    };
    match command {
        Command::Run => run(&options),
        Command::Bench => bench(&options),
        Command::Permissions => permissions(&options),
        Command::Approve => approve(&options),
        Command::Optional => optional(&options),
        Command::Remove => remove(&options),
        Command::Pack => pack(&options),
    }
}

struct Options {
    /// A package file, or for `remove` a plugin id.
    plugin: String,
    /// Already checked to be valid JSON. Only for `run` and `bench`.
    config: String,
    /// Only for `bench`.
    iterations: u32,
    /// The grant record given with `--grants`.
    grants: Option<PathBuf>,
    /// Only for `optional`: the permission, and whether to turn it on.
    optional: Option<(String, bool)>,
    /// Only for `pack`: the manifest file and the package to write.
    manifest: Option<PathBuf>,
    output: Option<PathBuf>,
}

fn parse(args: &[String], command: Command) -> Result<Options, String> {
    let renders = matches!(command, Command::Run | Command::Bench);
    let mut config = None;
    let mut iterations = DEFAULT_ITERATIONS;
    let mut grants = None;
    let mut manifest = None;
    let mut output = None;
    let mut positional = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            option @ ("--config" | "--config-file") if renders => {
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
            // pack checks and writes a package; it needs no grant record
            "--grants" if command != Command::Pack => {
                let value = args.next().ok_or("--grants needs a path")?;
                if grants.replace(PathBuf::from(value)).is_some() {
                    return Err("give --grants only once".to_owned());
                }
            }
            option @ ("--manifest" | "--output") if command == Command::Pack => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{option} needs a path"))?;
                let slot = if option == "--manifest" {
                    &mut manifest
                } else {
                    &mut output
                };
                if slot.replace(PathBuf::from(value)).is_some() {
                    return Err(format!("give {option} only once"));
                }
            }
            "--iterations" if command == Command::Bench => {
                iterations = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .filter(|&n| n > 0)
                    .ok_or("--iterations needs a whole number above 0")?;
            }
            option if option.starts_with("--") => {
                return Err(format!("unknown option `{option}`"));
            }
            argument => positional.push(argument.to_owned()),
        }
    }
    let mut positional = positional.into_iter();
    let plugin = positional.next().ok_or(match command {
        Command::Remove => "no plugin id given",
        Command::Pack => "no component file given",
        _ => "no plugin file given",
    })?;
    if command == Command::Pack && (manifest.is_none() || output.is_none()) {
        return Err("pack needs --manifest <file> and --output <file>".to_owned());
    }
    let optional = if command == Command::Optional {
        let permission = positional.next().ok_or("no permission given")?;
        let on = match positional.next().as_deref() {
            Some("on") => true,
            Some("off") => false,
            _ => return Err(format!("say whether to turn {permission} on or off")),
        };
        Some((permission, on))
    } else {
        None
    };
    // Required permissions are approved all together: approve takes no permission
    if let Some(argument) = positional.next() {
        return Err(format!("unexpected argument `{argument}`"));
    }
    let config = config.unwrap_or_else(|| "{}".to_owned());
    serde_json::from_str::<serde_json::Value>(&config)
        .map_err(|error| format!("the configuration is not valid JSON: {error}"))?;
    Ok(Options {
        plugin,
        config,
        iterations,
        grants,
        optional,
        manifest,
        output,
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

/// Shows what the plugin requests and what is granted to this file (FR-014); changes nothing.
fn permissions(options: &Options) -> ExitCode {
    let target = match Target::open(options) {
        Ok(target) => target,
        Err(status) => return status,
    };
    let checked = match target.check() {
        Ok(checked) => checked,
        Err(status) => return status,
    };
    let status = match target.grants.status(checked.manifest(), &target.origin) {
        Ok(status) => status,
        Err(error) => return refused(&error),
    };
    print!("{}", describe(&checked, &status));
    ExitCode::SUCCESS
}

/// The output of `permissions`, as `contracts/cli.md` shows it. Every text that comes from the
/// package or the record is escaped.
fn describe(checked: &CheckedPackage, status: &Status) -> String {
    let manifest = checked.manifest();
    let mut text = format!(
        "plugin: {} {}\nauthor: {}\ncontract: {}\n",
        escape(manifest.id().as_str()),
        escape(manifest.version()),
        escape(manifest.author()),
        checked.contract().as_str()
    );
    let approved = match &status.approval {
        None => "no".to_owned(),
        Some(entry) if !status.this_origin => {
            format!("no (approved from {})", escape(&entry.origin))
        }
        Some(entry) => format!(
            "version {} ({}) from {}",
            escape(&entry.approved_version),
            escape(&entry.approved_digest),
            escape(&entry.origin)
        ),
    };
    text.push_str(&format!("approved: {approved}\n"));
    if status.permissions.is_empty() {
        text.push_str("permissions: none\n");
    } else {
        text.push_str("permissions:\n");
        for permission in &status.permissions {
            text.push_str(&format!(
                "  {}  {}  {}\n",
                escape(&permission.name),
                if permission.required {
                    "required"
                } else {
                    "optional"
                },
                if permission.granted {
                    "granted"
                } else {
                    "not granted"
                }
            ));
        }
    }
    let providers = manifest.login_providers();
    if providers.is_empty() {
        text.push_str("login providers: none\n");
    } else {
        let providers: Vec<String> = providers.iter().map(|name| escape(name)).collect();
        text.push_str(&format!(
            "login providers: {} {LOGIN_PROVIDERS_LATER}\n",
            providers.join(", ")
        ));
    }
    text
}

/// Grants every required permission the manifest declares, all at once (FR-018), after the
/// checks that need no grants (load steps 1–6): a package they refuse is granted nothing.
fn approve(options: &Options) -> ExitCode {
    let target = match Target::open(options) {
        Ok(target) => target,
        Err(status) => return status,
    };
    let checked = match target.check() {
        Ok(checked) => checked,
        Err(status) => return status,
    };
    let manifest = checked.manifest();
    let digest = package::digest(&target.bytes);
    let approval = match target.grants.approve(manifest, &target.origin, &digest) {
        Ok(approval) => approval,
        Err(error) => return refused(&error),
    };
    if let Some(replaced) = &approval.replaced {
        println!("replaced: approval from {}", escape(replaced));
    }
    println!(
        "approved {} {}",
        escape(manifest.id().as_str()),
        escape(manifest.version())
    );
    for permission in &approval.required {
        println!("granted: {}", escape(permission));
    }
    ExitCode::SUCCESS
}

/// How many times `bench` repeats the checks and the re-link, whose median it prints.
const CHECK_REPEATS: u32 = 100;

/// Times one load, then `iterations` render calls, each with its own instantiation. Then the
/// checks of a load without the compile, and a re-link after a grant change (SC-007), each as
/// the median of [`CHECK_REPEATS`] runs.
fn bench(options: &Options) -> ExitCode {
    let target = match Target::open(options) {
        Ok(target) => target,
        Err(status) => return status,
    };
    let bytes = target.bytes.clone();
    let start = Instant::now();
    let plugin = match target
        .runtime
        .load(&target.bytes, &target.origin, Arc::new(target.grants))
    {
        Ok(plugin) => plugin,
        Err(error) => return refused(&error),
    };
    let load_time = start.elapsed();

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

    let checks = match median(|| plugin.recheck(&bytes)) {
        Ok(time) => time,
        Err(error) => return refused(&error),
    };
    let relink = match median(|| plugin.relink()) {
        Ok(time) => time,
        Err(error) => return refused(&error),
    };

    println!("load_ms: {:.1}", load_time.as_secs_f64() * 1000.0);
    println!("render_p50_us: {}", percentile(&times, 50).as_micros());
    println!("render_p95_us: {}", percentile(&times, 95).as_micros());
    println!("render_max_us: {}", percentile(&times, 100).as_micros());
    println!("checks_us: {}", checks.as_micros());
    println!("relink_us: {}", relink.as_micros());
    ExitCode::SUCCESS
}

/// The median time of [`CHECK_REPEATS`] runs of `step`, or its first failure.
fn median(
    mut step: impl FnMut() -> Result<(), runtime::RuntimeError>,
) -> Result<Duration, runtime::RuntimeError> {
    let mut times = Vec::with_capacity(CHECK_REPEATS as usize);
    for _ in 0..CHECK_REPEATS {
        let start = Instant::now();
        step()?;
        times.push(start.elapsed());
    }
    times.sort();
    Ok(percentile(&times, 50))
}

/// The nearest-rank percentile of sorted, non-empty times.
fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    let rank = (percent * sorted.len()).div_ceil(100).max(1);
    sorted[rank - 1]
}

/// Turns one optional permission on or off for this file's approval, from the plugin's next
/// call (FR-014).
fn optional(options: &Options) -> ExitCode {
    let Some((permission, on)) = &options.optional else {
        return usage_error("no permission given");
    };
    let target = match Target::open(options) {
        Ok(target) => target,
        Err(status) => return status,
    };
    let checked = match target.check() {
        Ok(checked) => checked,
        Err(status) => return status,
    };
    if let Err(error) = checked.set_optional(&target.grants, &target.origin, permission, *on) {
        return refused(&error);
    }
    println!("{} {}", escape(permission), if *on { "on" } else { "off" });
    ExitCode::SUCCESS
}

/// Checks the manifest and writes the component with it added as its manifest section
/// (`contracts/package.md`). Compiles nothing and needs no grant record.
fn pack(options: &Options) -> ExitCode {
    let (Some(manifest_path), Some(output)) = (&options.manifest, &options.output) else {
        return usage_error("pack needs --manifest <file> and --output <file>");
    };
    let component = match read_plugin(&options.plugin, Limits::default().max_package_bytes) {
        Ok(component) => component,
        Err(message) => return usage_error(&message),
    };
    // At most one byte over the limit, which the manifest check then refuses
    let manifest_name = manifest_path.display().to_string();
    let manifest = match read_capped(&manifest_name, MAX_MANIFEST_BYTES, "manifest file") {
        Ok(manifest) => manifest,
        Err(message) => return usage_error(&message),
    };
    let packed = match package::pack(&component, &manifest) {
        Ok(packed) => packed,
        Err(error) => return refused(&error),
    };
    if let Err(error) = std::fs::write(output, &packed) {
        eprintln!(
            "error: cannot write {}: {error}",
            escape(&output.display().to_string())
        );
        return ExitCode::from(PLUGIN_FAILED);
    }
    println!("packed {}", escape(&output.display().to_string()));
    println!("digest: {}", package::digest(&packed));
    ExitCode::SUCCESS
}

/// Removes the plugin's approval and every grant with it, whatever file it was given for.
fn remove(options: &Options) -> ExitCode {
    let id = match PluginId::parse(&options.plugin) {
        Ok(id) => id,
        Err(problem) => return usage_error(&problem),
    };
    let grants = match grant_store(options.grants.as_ref()) {
        Ok(grants) => grants,
        Err(message) => return usage_error(&message),
    };
    match grants.remove(&id) {
        Ok(true) => println!("removed {id}"),
        Ok(false) => println!("{id} has no approval"),
        Err(error) => return refused(&error),
    }
    ExitCode::SUCCESS
}

/// Reads and loads a plugin with its grants, timing the load. On failure, prints why and
/// returns the exit status.
fn load(options: &Options) -> Result<(Plugin, Duration), ExitCode> {
    let target = Target::open(options)?;
    let start = Instant::now();
    let plugin = target
        .runtime
        .load(&target.bytes, &target.origin, Arc::new(target.grants))
        .map_err(|error| refused(&error))?;
    Ok((plugin, start.elapsed()))
}

/// A plugin file read for a command, with where it came from, the grant record and a runtime.
struct Target {
    bytes: Vec<u8>,
    origin: String,
    grants: GrantStore,
    runtime: Runtime,
}

impl Target {
    /// On failure, prints why and returns the exit status.
    fn open(options: &Options) -> Result<Target, ExitCode> {
        let limits = Limits::default();
        let path = &options.plugin;
        let bytes =
            read_plugin(path, limits.max_package_bytes).map_err(|message| usage_error(&message))?;
        let origin = origin(path).map_err(|message| usage_error(&message))?;
        let grants =
            grant_store(options.grants.as_ref()).map_err(|message| usage_error(&message))?;
        let runtime = Runtime::new(limits).map_err(|error| {
            eprintln!("error: cannot start the runtime: {error:#}");
            ExitCode::from(PLUGIN_FAILED)
        })?;
        Ok(Target {
            bytes,
            origin,
            grants,
            runtime,
        })
    }

    /// Load steps 1–6, which need no grants.
    fn check(&self) -> Result<CheckedPackage, ExitCode> {
        self.runtime
            .check(&self.bytes)
            .map_err(|error| refused(&error))
    }
}

/// Prints a refusal and returns the exit status for it.
fn refused(error: &runtime::RuntimeError) -> ExitCode {
    eprintln!("error: {error}");
    ExitCode::from(PLUGIN_FAILED)
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
    read_capped(path, max_bytes, "plugin file")
}

/// Reads at most `max_bytes + 1` bytes of the `what` at `path`, so the reader can tell it is too
/// large without reading it whole.
fn read_capped(path: &str, max_bytes: usize, what: &str) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("cannot read {what} {path}: {error}"))?;
    let mut bytes = Vec::new();
    file.take((max_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {what} {path}: {error}"))?;
    Ok(bytes)
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("error: {message}\n{USAGE}");
    ExitCode::from(USAGE_ERROR)
}
