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
//! For widgets with connectors (`specs/003-plugin-connectors/contracts/cli.md`): `permissions`
//! also shows each connector, its destinations and the operations the widget calls, with an
//! approval digest that `approve --expect <digest>` needs, so what is approved is what was
//! shown. `runtime update <plugin>` runs the widget's update and keeps the data it returns, and
//! `run` renders it from that data, never from the network. `runtime requests` prints the
//! request log. Kept data and the log live in the data directory (`--data <dir>`), and local
//! connectors in `--connectors <dir>`, which only this tool reads: it runs the runtime in
//! development mode.
//!
//! The tool is the host's control in this phase: it works in development mode only, on local
//! package files, with the development grant record (`--grants <path>`, or the default one in
//! the system's data directory). A package's origin is its file's canonical path, and an
//! approval applies only to the file it was given for: another file that copies an approved
//! plugin's id inherits none of its grants (FR-020).

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use runtime::connector::{Kind, ParameterType, Template};
use runtime::grants::{GrantStore, Status};
use runtime::manifest::MAX_MANIFEST_BYTES;
use runtime::manifest::PluginId;
use runtime::requests::{Entry, Outcome};
use runtime::{CheckedPackage, Limits, Plugin, Runtime, RuntimeOptions, escape, package};

const USAGE: &str = "usage: runtime run <plugin> [--config <json> | --config-file <path>] [--grants <path>] [--data <dir>] [--connectors <dir>]
       runtime update <plugin> [--config <json> | --config-file <path>] [--grants <path>] [--data <dir>] [--connectors <dir>]
       runtime bench <plugin> [--iterations <n>] [--kept-data <file>] [--config <json> | --config-file <path>] [--grants <path>] [--data <dir>] [--connectors <dir>]
       runtime permissions <plugin> [--grants <path>] [--data <dir>] [--connectors <dir>]
       runtime approve <plugin> [--expect <digest>] [--grants <path>] [--data <dir>] [--connectors <dir>]
       runtime optional <plugin> <permission | connector-id> on|off [--grants <path>] [--data <dir>] [--connectors <dir>]
       runtime remove <plugin-id> [--grants <path>] [--data <dir>]
       runtime requests [--plugin <plugin-id>] [--last <n>] [--data <dir>]
       runtime check-connector <file>
       runtime pack <component> --manifest <file> --output <file>";

/// Exit status when the plugin or the change was refused, or the plugin failed or was stopped.
const PLUGIN_FAILED: u8 = 1;
/// Exit status for usage errors: unknown command, missing file, invalid configuration.
const USAGE_ERROR: u8 = 2;

/// Render calls `bench` measures when `--iterations` is not given.
const DEFAULT_ITERATIONS: u32 = 1000;

/// Entries `requests` prints when `--last` is not given.
const DEFAULT_LAST: usize = 50;

/// What `permissions` says of login providers until accounts can be connected.
const LOGIN_PROVIDERS_LATER: &str = "(connecting accounts arrives in feature 005)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Run,
    Update,
    Requests,
    CheckConnector,
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
        Some("update") => Command::Update,
        Some("requests") => Command::Requests,
        Some("check-connector") => Command::CheckConnector,
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
        Command::Update => update(&options),
        Command::Requests => requests(&options),
        Command::CheckConnector => check_connector(&options),
        Command::Bench => bench(&options),
        Command::Permissions => permissions(&options),
        Command::Approve => approve(&options),
        Command::Optional => optional(&options),
        Command::Remove => remove(&options),
        Command::Pack => pack(&options),
    }
}

struct Options {
    /// A package file, or for `remove` a plugin id; empty for `requests`.
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
    /// The data directory given with `--data`: kept data and the request log.
    data: Option<PathBuf>,
    /// Where local connectors are read, given with `--connectors`.
    connectors: Option<PathBuf>,
    /// Only for `approve`: the approval digest `permissions` printed.
    expect: Option<String>,
    /// Only for `requests`: whose entries, and how many.
    plugin_filter: Option<String>,
    last: usize,
    /// Only for `bench`: a JSON document to render from as kept data.
    kept_data: Option<serde_json::Value>,
}

fn parse(args: &[String], command: Command) -> Result<Options, String> {
    let renders = matches!(command, Command::Run | Command::Bench | Command::Update);
    // Which commands read grants, data and local connectors (contracts/cli.md)
    let reads_files_only = matches!(command, Command::Pack | Command::CheckConnector);
    let uses_grants = !reads_files_only && command != Command::Requests;
    let uses_data = !reads_files_only;
    let uses_connectors = uses_grants && command != Command::Remove;
    let mut config = None;
    let mut data = None;
    let mut connectors = None;
    let mut expect = None;
    let mut plugin_filter = None;
    let mut last = DEFAULT_LAST;
    let mut kept_data = None;
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
            "--grants" if uses_grants => {
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
            option @ ("--data" | "--connectors")
                if (option == "--data" && uses_data) || uses_connectors =>
            {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{option} needs a directory"))?;
                let slot = if option == "--data" {
                    &mut data
                } else {
                    &mut connectors
                };
                if slot.replace(PathBuf::from(value)).is_some() {
                    return Err(format!("give {option} only once"));
                }
            }
            "--expect" if command == Command::Approve => {
                let value = args.next().ok_or("--expect needs the approval digest")?;
                if expect.replace(value.clone()).is_some() {
                    return Err("give --expect only once".to_owned());
                }
            }
            "--plugin" if command == Command::Requests => {
                let value = args.next().ok_or("--plugin needs a plugin id")?;
                PluginId::parse(value)?;
                plugin_filter = Some(value.clone());
            }
            "--last" if command == Command::Requests => {
                last = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .filter(|&n| n > 0)
                    .ok_or("--last needs a whole number above 0")?;
            }
            "--kept-data" if command == Command::Bench => {
                let path = args.next().ok_or("--kept-data needs a file")?;
                let text = std::fs::read_to_string(path)
                    .map_err(|error| format!("cannot read kept data file {path}: {error}"))?;
                let data = serde_json::from_str(&text).map_err(|error| {
                    format!("the kept data in {path} is not valid JSON: {error}")
                })?;
                kept_data = Some(data);
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
    let plugin = if command == Command::Requests {
        String::new()
    } else {
        positional.next().ok_or(match command {
            Command::Remove => "no plugin id given",
            Command::Pack => "no component file given",
            Command::CheckConnector => "no connector file given",
            _ => "no plugin file given",
        })?
    };
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
        data,
        connectors,
        expect,
        plugin_filter,
        last,
        kept_data,
    })
}

fn run(options: &Options) -> ExitCode {
    let plugin = match load(options) {
        Ok((plugin, _)) => plugin,
        Err(status) => return status,
    };

    let report = plugin.render(&options.config);
    print_logs(&report.logs, report.dropped_logs);
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

fn print_logs(logs: &[runtime::LogMessage], dropped: u32) {
    for log in logs {
        eprintln!("[plugin] {}: {}", log.level.as_str(), log.message);
    }
    if dropped > 0 {
        eprintln!("[runtime] {dropped} log messages dropped");
    }
}

/// Runs the widget's update and keeps the data it returns (FR-010, FR-012). A call that is not
/// granted, refused or failed does not fail the update: the widget sees it and decides.
fn update(options: &Options) -> ExitCode {
    let plugin = match load(options) {
        Ok((plugin, _)) => plugin,
        Err(status) => return status,
    };
    let report = plugin.update(&options.config);
    print_logs(&report.logs, report.dropped_logs);
    match report.result {
        Ok(_) => {
            let manifest = plugin.manifest();
            println!(
                "updated {} {}",
                escape(manifest.id().as_str()),
                escape(manifest.version())
            );
            let calls = report.calls;
            println!(
                "calls: {} ({} not granted, {} refused, {} failed), {} bytes received",
                calls.calls, calls.not_granted, calls.refused, calls.failed, calls.received_bytes
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error} (previous data kept)");
            ExitCode::from(PLUGIN_FAILED)
        }
    }
}

/// Prints the request log, oldest first, one line per entry, every text a plugin or a connector
/// wrote escaped (FR-009). Changes nothing.
fn requests(options: &Options) -> ExitCode {
    let data = match data_dir(options) {
        Ok(data) => data,
        Err(message) => return usage_error(&message),
    };
    let runtime = match Runtime::new(Limits::default(), RuntimeOptions::new(data)) {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("error: cannot start the runtime: {error:#}");
            return ExitCode::from(PLUGIN_FAILED);
        }
    };
    match runtime.read_requests(options.last, options.plugin_filter.as_deref()) {
        Ok(entries) => {
            for entry in &entries {
                println!("{}", request_line(entry));
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: cannot read the request log: {error}");
            ExitCode::from(PLUGIN_FAILED)
        }
    }
}

/// One entry of the request log as `requests` prints it (contracts/cli.md).
fn request_line(entry: &Entry) -> String {
    let mut line = utc(entry.at_ms);
    line.push(' ');
    line.push_str(&escape(&entry.plugin));
    if let Some(summary) = &entry.summary {
        line.push_str(&format!(
            " … {} more calls past the limit of this run (not-granted: {}, refused: {})",
            summary.calls, summary.not_granted, summary.refused
        ));
        return line;
    }
    line.push_str(&format!(
        " {} {}",
        escape(&entry.connector),
        escape(&entry.operation)
    ));
    if let (Some(method), Some(host), Some(path)) = (&entry.method, &entry.host, &entry.path) {
        line.push_str(&format!(
            " {} {} {}",
            escape(method),
            escape(host),
            escape(path)
        ));
    }
    match &entry.outcome {
        Outcome::Status { status } => line.push_str(&format!(
            " {status} {}B {}ms",
            entry.received_bytes, entry.duration_ms
        )),
        Outcome::NotGranted { .. } => line.push_str(" not-granted"),
        Outcome::Refused { refused, detail } => {
            line.push_str(&format!(" refused {}: {}", escape(refused), escape(detail)));
        }
        Outcome::Failed { failed, detail } => {
            line.push_str(&format!(" failed {}: {}", escape(failed), escape(detail)));
        }
    }
    if let Some(redirect) = entry.redirect_of {
        line.push_str(&format!(" (redirect {redirect})"));
    }
    line
}

/// `2026-10-23T14:02:11.204Z`: milliseconds since the Unix epoch in UTC, without a date library
/// (the civil-from-days algorithm of Howard Hinnant's date algorithms).
fn utc(at_ms: u64) -> String {
    let seconds = at_ms / 1000;
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let time = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        time / 3600,
        time % 3600 / 60,
        time % 60,
        at_ms % 1000
    )
}

/// Checks a connector file as the host would load it as a local connector, for authors:
/// `<name>.json` must carry `local/<name>`, and no host the catalog reserves (contracts/cli.md).
fn check_connector(options: &Options) -> ExitCode {
    let path = Path::new(&options.plugin);
    let bytes = match read_capped(
        &options.plugin,
        runtime::connector::MAX_CONNECTOR_BYTES,
        "connector file",
    ) {
        Ok(bytes) => bytes,
        Err(message) => return usage_error(&message),
    };
    let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return usage_error("the connector file has no UTF-8 name");
    };
    // Nothing is written: the data directory is only where a runtime would keep its data
    let runtime = match Runtime::new(Limits::default(), RuntimeOptions::new(std::env::temp_dir())) {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("error: cannot start the runtime: {error:#}");
            return ExitCode::from(PLUGIN_FAILED);
        }
    };
    let connector = match runtime.check_local_connector(&bytes, name) {
        Ok(connector) => connector,
        Err(error) => return refused(&error),
    };
    let plural = |count: usize, one: &str| {
        if count == 1 {
            format!("{count} {one}")
        } else {
            format!("{count} {one}s")
        }
    };
    let operations: Vec<_> = connector.operations().collect();
    let reads = operations
        .iter()
        .filter(|(_, operation)| operation.kind() == Kind::Read)
        .count();
    let changes = operations.len() - reads;
    let mut kinds = vec![plural(reads, "read")];
    if changes > 0 {
        kinds.push(plural(changes, "change"));
    }
    println!(
        "ok: {} {}, {}, {} ({})",
        escape(connector.id()),
        escape(&connector.version().to_string()),
        plural(connector.destinations().len(), "destination"),
        plural(operations.len(), "operation"),
        kinds.join(", ")
    );
    ExitCode::SUCCESS
}

/// Shows what the plugin requests and what is granted to this file (FR-014); changes nothing.
fn permissions(options: &Options) -> ExitCode {
    let target = match Target::open(options) {
        Ok(target) => target,
        Err(status) => return status,
    };
    // Connectors that cannot be used are shown in their place, not as a refusal of the whole
    let checked = match target.runtime.inspect(&target.bytes) {
        Ok(checked) => checked,
        Err(error) => return refused(&error),
    };
    let status = match target.grants.status(checked.manifest(), &target.origin) {
        Ok(status) => status,
        Err(error) => return refused(&error),
    };
    let entry = status.approval.clone().filter(|_| status.this_origin);
    print!("{}", describe(&checked, &status, entry.as_ref()));
    ExitCode::SUCCESS
}

/// The output of `permissions`, as `contracts/cli.md` shows it. Every text that comes from the
/// package or the record is escaped.
fn describe(
    checked: &CheckedPackage,
    status: &Status,
    entry: Option<&runtime::grants::Entry>,
) -> String {
    let manifest = checked.manifest();
    let mut text = format!(
        "plugin: {} {}\nauthor: {}\ncontract: {}\n",
        escape(manifest.id().as_str()),
        escape(manifest.version()),
        escape(manifest.author()),
        checked.contract_version().display()
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
    let declares_connectors = !manifest.connectors().is_empty();
    if declares_connectors {
        text.push_str(&describe_connectors(checked, entry));
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
    // The digest of what `approve` would grant, only when it could grant it
    if declares_connectors && checked.refused_connectors().is_empty() {
        text.push_str(&format!("approval digest: {}\n", checked.approval_digest()));
    }
    text
}

/// The `connectors` section of `permissions` (contracts/cli.md): each connector, where it comes
/// from, every destination, and each operation the widget calls with the parameters it sends
/// and the fixed values the connector writes; then whether data read from this machine can
/// reach another destination.
fn describe_connectors(checked: &CheckedPackage, entry: Option<&runtime::grants::Entry>) -> String {
    let mut text = String::from("connectors:\n");
    let manifest = checked.manifest();
    for request in manifest.connectors() {
        let required = if request.required {
            "required"
        } else {
            "optional"
        };
        if let Some((_, error)) = checked
            .refused_connectors()
            .iter()
            .find(|(id, _)| *id == request.id)
        {
            text.push_str(&format!(
                "  {}  {required}  error: {error}\n",
                escape(&request.id)
            ));
            continue;
        }
        let Some(loaded) = checked
            .connectors()
            .iter()
            .find(|loaded| loaded.id() == request.id)
        else {
            continue;
        };
        let connector = loaded.connector();
        let granted = if entry.is_some_and(|entry| entry.is_connector_granted(&request.id)) {
            "granted"
        } else {
            "not granted"
        };
        text.push_str(&format!(
            "  {} {}  {required}  {granted}\n",
            escape(&request.id),
            escape(&connector.version().to_string())
        ));
        let catalog = loaded.origin() == "catalog";
        if catalog {
            text.push_str("    from: official catalog\n");
        } else {
            text.push_str(&format!(
                "    from: local file {} (development)\n",
                escape(loaded.origin())
            ));
        }
        let destinations: Vec<String> = connector
            .destinations()
            .iter()
            .map(|origin| escape(&origin.serialization()))
            .collect();
        text.push_str(&format!("    destinations: {}\n", destinations.join(", ")));
        for name in loaded.operations() {
            let Some(operation) = connector.operation(name) else {
                continue;
            };
            let who = if catalog {
                "reviewed"
            } else {
                "declared by the developer"
            };
            let kind = match operation.kind() {
                Kind::Read => "read",
                Kind::Change => "change",
            };
            let origin = connector
                .destination(operation.destination())
                .map(|origin| origin.serialization())
                .unwrap_or_default();
            let mut path = String::new();
            for segment in operation.path() {
                path.push('/');
                match segment {
                    Template::Literal(text) => path.push_str(text),
                    Template::Parameter(parameter) => path.push_str(&format!("{{{parameter}}}")),
                }
            }
            text.push_str(&format!(
                "    {}  {kind} ({who})  {} {}\n",
                escape(name),
                operation.method().as_str(),
                escape(&format!("{origin}{path}"))
            ));
            text.push_str(&format!("      sends: {}\n", sends(operation)));
            let fixed = fixed(operation);
            if !fixed.is_empty() {
                text.push_str(&format!("      fixed: {}\n", fixed.join(", ")));
            }
        }
    }
    if let Some(flow) = data_flow(checked) {
        text.push_str(&flow);
    }
    text
}

/// The parameters an operation sends: those in its path first, in order, then the rest by name.
fn sends(operation: &runtime::connector::Operation) -> String {
    let parameters = operation.parameters();
    if parameters.is_empty() {
        return "nothing".to_owned();
    }
    let mut names: Vec<&String> = operation
        .path()
        .iter()
        .filter_map(|segment| match segment {
            Template::Parameter(name) => parameters.get_key_value(name).map(|(name, _)| name),
            Template::Literal(_) => None,
        })
        .collect();
    for name in parameters.keys() {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let described: Vec<String> = names
        .into_iter()
        .map(|name| {
            let parameter = &parameters[name];
            let kind = match parameter.kind {
                ParameterType::Text { .. } => "text",
                ParameterType::Integer { .. } => "integer",
                ParameterType::Flag => "flag",
            };
            if parameter.required {
                format!("{} ({kind})", escape(name))
            } else {
                format!("{} ({kind}, optional)", escape(name))
            }
        })
        .collect();
    described.join(", ")
}

/// Every value the connector writes into the request itself: they are public.
fn fixed(operation: &runtime::connector::Operation) -> Vec<String> {
    let mut fixed: Vec<String> = operation
        .query()
        .iter()
        .filter_map(|(name, value)| match value {
            Template::Literal(value) => Some(format!("{}={}", escape(name), escape(value))),
            Template::Parameter(_) => None,
        })
        .collect();
    fixed.extend(
        operation
            .headers()
            .iter()
            .map(|(name, value)| format!("{}: {}", escape(name), escape(value))),
    );
    if let Some(body) = operation.body() {
        fixed_body(body, "body", &mut fixed);
    }
    fixed
}

fn fixed_body(value: &serde_json::Value, path: &str, fixed: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (name, item) in fields {
                fixed_body(item, &format!("{path}.{name}"), fixed);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                fixed_body(item, &format!("{path}[{index}]"), fixed);
            }
        }
        serde_json::Value::String(text) if runtime::connector::is_placeholder(text) => {}
        other => fixed.push(format!("{}: {}", escape(path), escape(&other.to_string()))),
    }
}

/// When a local connector reaches this machine and the widget can also reach another
/// destination, a line says so (Principle III).
fn data_flow(checked: &CheckedPackage) -> Option<String> {
    let mut here = Vec::new();
    let mut elsewhere = Vec::new();
    for loaded in checked.connectors() {
        for origin in loaded.connector().destinations() {
            let serialized = escape(&origin.serialization());
            let list = if origin.is_https() {
                &mut elsewhere
            } else {
                &mut here
            };
            if !list.contains(&serialized) {
                list.push(serialized);
            }
        }
    }
    (!here.is_empty() && !elsewhere.is_empty()).then(|| {
        format!(
            "data flow: data read from this machine ({}) can reach {}\n",
            here.join(", "),
            elsewhere.join(", ")
        )
    })
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
    if !manifest.connectors().is_empty() && options.expect.is_none() {
        return usage_error(&format!(
            "approve needs --expect <digest> for a widget that declares connectors: run `runtime permissions {}` and give the approval digest it prints",
            options.plugin
        ));
    }
    let approval = match checked.approve(&target.grants, &target.origin, options.expect.as_deref())
    {
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
    if !checked.connectors().is_empty() {
        let connectors: Vec<String> = checked
            .connectors()
            .iter()
            .map(|loaded| {
                let destinations: Vec<String> = loaded
                    .connector()
                    .destinations()
                    .iter()
                    .map(|origin| escape(&origin.serialization()))
                    .collect();
                format!("{} ({})", escape(loaded.id()), destinations.join(", "))
            })
            .collect();
        println!("connectors: {}", connectors.join(", "));
    }
    ExitCode::SUCCESS
}

/// How many times `bench` repeats the checks and the re-link, whose median it prints.
const CHECK_REPEATS: u32 = 100;

/// Times one load, then `iterations` render calls, each with its own instantiation. Then the
/// checks of a load without the compile, connectors included, and a re-link after a grant change
/// (SC-007), and with `--kept-data` a render from kept data (SC-004), each as the median of
/// [`CHECK_REPEATS`] runs.
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
    // A connected widget rendering from kept data held in memory: no update, no request
    let render_with_data = match &options.kept_data {
        None => None,
        Some(data) => match median(|| {
            plugin
                .render_with_kept(&options.config, data)
                .result
                .map(drop)
        }) {
            Ok(time) => Some(time),
            Err(error) => return refused(&error),
        },
    };

    println!("load_ms: {:.1}", load_time.as_secs_f64() * 1000.0);
    println!("render_p50_us: {}", percentile(&times, 50).as_micros());
    println!("render_p95_us: {}", percentile(&times, 95).as_micros());
    println!("render_max_us: {}", percentile(&times, 100).as_micros());
    println!("checks_us: {}", checks.as_micros());
    println!("relink_us: {}", relink.as_micros());
    if let Some(time) = render_with_data {
        println!("render_with_data_us: {}", time.as_micros());
    }
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
    // A connector id has a `/`; a permission name never does
    if permission.contains('/') {
        if let Err(error) = checked.set_connector(&target.grants, &target.origin, permission, *on) {
            return refused(&error);
        }
        if *on {
            println!("{} on", escape(permission));
        } else {
            // Data fetched with the connector on goes with it (FR-010)
            if let Err(error) = target.runtime.discard_kept_data(checked.manifest().id()) {
                eprintln!("error: cannot discard the kept data: {error}");
                return ExitCode::from(PLUGIN_FAILED);
            }
            println!("{} off (kept data discarded)", escape(permission));
        }
        return ExitCode::SUCCESS;
    }
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
    // The data the host kept for it goes with the approval (FR-010)
    let data = match data_dir(options) {
        Ok(data) => data,
        Err(message) => return usage_error(&message),
    };
    match Runtime::new(Limits::default(), RuntimeOptions::new(data)) {
        Ok(runtime) => {
            if let Err(error) = runtime.discard_kept_data(&id) {
                eprintln!("error: cannot discard the kept data of {id}: {error}");
                return ExitCode::from(PLUGIN_FAILED);
            }
        }
        Err(error) => {
            eprintln!("error: cannot start the runtime: {error:#}");
            return ExitCode::from(PLUGIN_FAILED);
        }
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
        let data_dir = data_dir(options).map_err(|message| usage_error(&message))?;
        // The development tool, the only place local connectors load (FR-018)
        let mut runtime_options = RuntimeOptions::new(data_dir).development(true);
        if let Some(connectors) = &options.connectors {
            runtime_options = runtime_options.connectors_dir(connectors);
        }
        let runtime = Runtime::new(limits, runtime_options).map_err(|error| {
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

/// The data directory given with `--data`, or the default development one.
fn data_dir(options: &Options) -> Result<PathBuf, String> {
    match &options.data {
        Some(data) => Ok(data.clone()),
        None => RuntimeOptions::default_development_data_dir().ok_or_else(|| {
            "this system has no data directory for kept data and the request log; give --data <dir>"
                .to_owned()
        }),
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
