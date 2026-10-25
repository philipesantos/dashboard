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
//! For widgets with storage (`specs/004-plugin-storage/contracts/cli.md`): `permissions` shows
//! the quota declared, the quota approved and the store's usage, with the approval digest that
//! `approve --expect` needs, and `update` says what the update did with its store. Stores live
//! in `<data dir>/storage/`: `runtime storage list` lists them, `runtime storage show <plugin>`
//! prints one, every text escaped, and `runtime storage clear` empties one, by package or by id
//! and origin when the package file is gone.
//!
//! For connectors with a credential (`specs/005-plugin-credentials/contracts/cli.md`):
//! `runtime accounts connect` signs in to a connector's account in the system browser, or reads
//! its key from the terminal without echo, into the development vault in the system's keychain;
//! `runtime accounts list` lists what is connected and `runtime accounts disconnect` deletes it
//! and revokes it. No command prints, reads from a file or takes as an argument any credential.
//! `permissions` shows each credential and whether the widget acts as the account.
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

use runtime::accounts::{Account, AccountKind, Connected, Reason};
use runtime::connector::{Access, Credential, CredentialKind, Kind, ParameterType, Template};
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
       runtime storage list [--grants <path>] [--data <dir>]
       runtime storage show <plugin> [--grants <path>] [--data <dir>] [--connectors <dir>]
       runtime storage clear <plugin> | --id <plugin-id> --origin <path> [--grants <path>] [--data <dir>]
       runtime accounts connect <connector-id> [--origin <path>] [--client <id>] [--no-browser] [--grants <path>] [--data <dir>] [--connectors <dir>]
       runtime accounts list [--grants <path>] [--data <dir>] [--connectors <dir>]
       runtime accounts disconnect <connector-id> [--origin <path>] [--grants <path>] [--data <dir>] [--connectors <dir>]
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

/// What `permissions` says of login providers: the field grants nothing since feature 005.
const LOGIN_PROVIDERS_OBSOLETE: &str = "(obsolete: accounts come from connectors)";

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
    StorageList,
    StorageShow,
    StorageClear,
    AccountsConnect,
    AccountsList,
    AccountsDisconnect,
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
        Some("storage") => match args.get(1).map(String::as_str) {
            Some("list") => Command::StorageList,
            Some("show") => Command::StorageShow,
            Some("clear") => Command::StorageClear,
            Some(other) => return usage_error(&format!("unknown storage command `{other}`")),
            None => return usage_error("storage needs list, show or clear"),
        },
        Some("accounts") => match args.get(1).map(String::as_str) {
            Some("connect") => Command::AccountsConnect,
            Some("list") => Command::AccountsList,
            Some("disconnect") => Command::AccountsDisconnect,
            Some(other) => return usage_error(&format!("unknown accounts command `{other}`")),
            None => return usage_error("accounts needs connect, list or disconnect"),
        },
        Some(command) => return usage_error(&format!("unknown command `{command}`")),
        None => return usage_error("no command given"),
    };
    // A storage or accounts command's arguments start after its subcommand
    let skip = if matches!(
        command,
        Command::StorageList
            | Command::StorageShow
            | Command::StorageClear
            | Command::AccountsConnect
            | Command::AccountsList
            | Command::AccountsDisconnect
    ) {
        2
    } else {
        1
    };
    let options = match parse(&args[skip..], command) {
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
        Command::StorageList => storage_list(&options),
        Command::StorageShow => storage_show(&options),
        Command::StorageClear => storage_clear(&options),
        Command::AccountsConnect => accounts_connect(&options),
        Command::AccountsList => accounts_list(&options),
        Command::AccountsDisconnect => accounts_disconnect(&options),
    }
}

struct Options {
    /// A package file, for `remove` a plugin id, for `accounts connect` and `disconnect` a
    /// connector id; empty for `requests`, `storage list` and `accounts list`.
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
    /// Only for `storage clear`: the store's plugin id and origin, for a package file that is
    /// gone.
    store: Option<(String, String)>,
    /// Only for `accounts connect` and `disconnect`: the connector's origin, given with
    /// `--origin`.
    account_origin: Option<String>,
    /// Only for `accounts connect`: the client registration, and whether to open the browser.
    client: Option<String>,
    no_browser: bool,
}

fn parse(args: &[String], command: Command) -> Result<Options, String> {
    let renders = matches!(command, Command::Run | Command::Bench | Command::Update);
    // Which commands read grants, data and local connectors (contracts/cli.md)
    let reads_files_only = matches!(command, Command::Pack | Command::CheckConnector);
    let uses_grants = !reads_files_only && command != Command::Requests;
    let uses_data = !reads_files_only;
    let uses_connectors = uses_grants
        && !matches!(
            command,
            Command::Remove | Command::StorageList | Command::StorageClear
        );
    let names_account = matches!(
        command,
        Command::AccountsConnect | Command::AccountsDisconnect
    );
    let mut account_origin = None;
    let mut client = None;
    let mut no_browser = false;
    let mut store_id = None;
    let mut store_origin = None;
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
            option @ ("--id" | "--origin") if command == Command::StorageClear => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{option} needs a value"))?;
                let slot = if option == "--id" {
                    &mut store_id
                } else {
                    &mut store_origin
                };
                if slot.replace(value.clone()).is_some() {
                    return Err(format!("give {option} only once"));
                }
            }
            "--origin" if names_account => {
                let value = args.next().ok_or("--origin needs a path")?;
                if account_origin.replace(value.clone()).is_some() {
                    return Err("give --origin only once".to_owned());
                }
            }
            "--client" if command == Command::AccountsConnect => {
                let value = args.next().ok_or("--client needs a client id")?;
                if client.replace(value.clone()).is_some() {
                    return Err("give --client only once".to_owned());
                }
            }
            "--no-browser" if command == Command::AccountsConnect => no_browser = true,
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
    let store = match (store_id, store_origin) {
        (Some(id), Some(origin)) => {
            PluginId::parse(&id)?;
            Some((id, origin))
        }
        (None, None) => None,
        _ => {
            return Err("storage clear needs both --id <plugin-id> and --origin <path>".to_owned());
        }
    };
    let mut positional = positional.into_iter();
    let plugin = if command == Command::Requests
        || command == Command::StorageList
        || command == Command::AccountsList
        || (command == Command::StorageClear && store.is_some())
    {
        String::new()
    } else {
        positional.next().ok_or(match command {
            Command::Remove => "no plugin id given",
            Command::Pack => "no component file given",
            Command::CheckConnector => "no connector file given",
            Command::AccountsConnect | Command::AccountsDisconnect => "no connector id given",
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
        store,
        account_origin,
        client,
        no_browser,
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
            if !manifest.connectors().is_empty() {
                let calls = &report.calls;
                println!(
                    "calls: {} ({} not granted, {} refused, {} failed), {} bytes received",
                    calls.calls,
                    calls.not_granted,
                    calls.refused,
                    calls.failed,
                    calls.received_bytes
                );
                for id in &calls.needs_connecting {
                    println!(
                        "account: {} needs connecting again (runtime accounts connect {})",
                        escape(id),
                        escape(id)
                    );
                }
            }
            if let Some(storage) = report.storage {
                println!(
                    "storage: {} calls, {} writes kept, used {} of {} bytes",
                    storage.calls, storage.writes_kept, storage.used_bytes, storage.quota_bytes
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            // A storage failure already says nothing it wrote was kept; any other failure of a
            // widget with storage says so here (specs/004-plugin-storage/contracts/cli.md)
            let kept = if plugin.manifest().storage().is_some()
                && error.kind() != runtime::ErrorKind::StorageFailed
            {
                "storage writes not kept, previous data kept"
            } else {
                "previous data kept"
            };
            eprintln!("error: {error} ({kept})");
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
    line.push(' ');
    line.push_str(&escape(&entry.connector));
    if let Some(account) = &entry.account {
        line.push_str(&format!(" as {}", escape(account)));
    }
    line.push(' ');
    line.push_str(&escape(&entry.operation));
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
    let used = checked.manifest().storage().map(|_| {
        target
            .runtime
            .storage_used_bytes(checked.manifest().id(), &target.origin)
    });
    // An account that cannot be read is shown as none connected
    let accounts = target.runtime.list_accounts().unwrap_or_default();
    print!(
        "{}",
        describe(&checked, &status, entry.as_ref(), used, &accounts)
    );
    ExitCode::SUCCESS
}

/// The output of `permissions`, as `contracts/cli.md` shows it. Every text that comes from the
/// package or the record is escaped.
fn describe(
    checked: &CheckedPackage,
    status: &Status,
    entry: Option<&runtime::grants::Entry>,
    used: Option<Result<u64, runtime::RuntimeError>>,
    accounts: &[Account],
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
    if let (Some(storage), Some(used)) = (manifest.storage(), used) {
        text.push_str(&describe_storage(storage, entry, used));
    }
    let declares_connectors = !manifest.connectors().is_empty();
    if declares_connectors {
        text.push_str(&describe_connectors(checked, entry, accounts));
    }
    let providers = manifest.login_providers();
    if providers.is_empty() {
        text.push_str("login providers: none\n");
    } else {
        let providers: Vec<String> = providers.iter().map(|name| escape(name)).collect();
        text.push_str(&format!(
            "login providers: {} {LOGIN_PROVIDERS_OBSOLETE}\n",
            providers.join(", ")
        ));
    }
    // The digest of what `approve` would grant, only when it could grant it
    if (declares_connectors || manifest.storage().is_some())
        && checked.refused_connectors().is_empty()
    {
        text.push_str(&format!("approval digest: {}\n", checked.approval_digest()));
    }
    text
}

/// The `storage:` line of `permissions`: the quota declared, the one the last approval showed,
/// and the store's usage (`specs/004-plugin-storage/contracts/cli.md`).
fn describe_storage(
    storage: runtime::manifest::StorageRequest,
    entry: Option<&runtime::grants::Entry>,
    used: Result<u64, runtime::RuntimeError>,
) -> String {
    let approved = entry.and_then(|entry| entry.storage_quota_bytes);
    let approval = match approved {
        Some(approved) => format!("approved up to {approved} bytes"),
        None => "not approved".to_owned(),
    };
    let used = match used {
        Ok(bytes) => format!("used {bytes} bytes"),
        Err(error) => format!("usage unknown ({error})"),
    };
    let above = match approved {
        Some(approved) if storage.quota_bytes > approved => {
            " (above the approved quota: approve again)"
        }
        _ => "",
    };
    format!(
        "storage: quota {} bytes{}, {approval}, {used}{above}\n",
        storage.quota_bytes,
        size_in_units(storage.quota_bytes)
    )
}

/// ` (64 KiB)` or ` (8 MiB)` for a whole number of either, otherwise nothing.
fn size_in_units(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    if bytes >= MIB && bytes.is_multiple_of(MIB) {
        format!(" ({} MiB)", bytes / MIB)
    } else if bytes >= KIB && bytes.is_multiple_of(KIB) {
        format!(" ({} KiB)", bytes / KIB)
    } else {
        String::new()
    }
}

/// The `connectors` section of `permissions` (contracts/cli.md): each connector, where it comes
/// from, every destination, and each operation the widget calls with the parameters it sends
/// and the fixed values the connector writes; then whether data read from this machine can
/// reach another destination.
fn describe_connectors(
    checked: &CheckedPackage,
    entry: Option<&runtime::grants::Entry>,
    accounts: &[Account],
) -> String {
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
        if let Some(credential) = connector.credential() {
            let account = accounts.iter().find(|account| {
                account.connector == request.id && account.origin == loaded.origin()
            });
            text.push_str(&describe_credential(
                credential,
                catalog,
                request.account,
                account,
            ));
        }
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
    text.push_str(&account_data_flow(checked));
    text
}

/// The `credential:` block of a connector in `permissions` (`specs/005-plugin-credentials/
/// contracts/cli.md`): what is sent where, what the account lets the connector do, and whether
/// this widget acts as the account.
fn describe_credential(
    credential: &Credential,
    catalog: bool,
    uses: bool,
    account: Option<&Account>,
) -> String {
    let kind = match credential.kind() {
        CredentialKind::Account => "account",
        CredentialKind::Key => "key",
    };
    let required = if credential.required() {
        "required"
    } else {
        "optional"
    };
    let header = match credential.scheme() {
        Some(scheme) => format!("{}: {}", escape(credential.header()), escape(scheme)),
        None => escape(credential.header()),
    };
    let origins: Vec<String> = credential
        .origins()
        .iter()
        .map(|origin| escape(&origin.serialization()))
        .collect();
    let mut text = format!(
        "    credential: {kind} ({required}), {header}, sent to {}\n",
        origins.join(", ")
    );
    if let Some(declared) = credential.account() {
        text.push_str(&format!(
            "      sign-in: {}\n      token: {}\n",
            escape(declared.authorize().as_str()),
            escape(declared.token().as_str())
        ));
    }
    if !credential.access().is_empty() {
        let who = if catalog {
            "reviewed"
        } else {
            "declared by the developer"
        };
        text.push_str(&format!(
            "      asks for: {} ({who})\n",
            access_list(credential.access())
        ));
    }
    let acts_as = match (uses, account) {
        (false, _) => "this widget does not use the account".to_owned(),
        (true, Some(account)) => {
            format!(
                "this widget acts as the account: {}",
                account_label(account)
            )
        }
        (true, None) if credential.required() => {
            "this widget acts as the account: none connected (calls answer account not connected)"
                .to_owned()
        }
        (true, None) => {
            "this widget acts as the account: none connected (calls go without one)".to_owned()
        }
    };
    text.push_str(&format!("      {acts_as}\n"));
    text
}

/// `notes read, notes write`: what an account lets a connector do.
fn access_list(access: &[Access]) -> String {
    let items: Vec<String> = access
        .iter()
        .map(|item| format!("{} {}", escape(&item.name), item.level.as_str()))
        .collect();
    items.join(", ")
}

/// `ada (id 1234567)`, or the name alone when the service gave no id.
fn account_label(account: &Account) -> String {
    match &account.service_id {
        Some(id) => format!("{} (id {})", escape(&account.name), escape(id)),
        None => escape(&account.name),
    }
}

/// For each connector the widget uses with an account, a line naming every origin of its other
/// destinations that data read as the account can reach (research R14, Principle III).
fn account_data_flow(checked: &CheckedPackage) -> String {
    let mut text = String::new();
    for request in checked.manifest().connectors() {
        let Some(credential) = request
            .account
            .then(|| {
                checked
                    .connectors()
                    .iter()
                    .find(|loaded| loaded.id() == request.id)
                    .and_then(|loaded| loaded.connector().credential())
            })
            .flatten()
        else {
            continue;
        };
        let mut elsewhere: Vec<String> = Vec::new();
        for loaded in checked.connectors() {
            for origin in loaded.connector().destinations() {
                let serialized = escape(&origin.serialization());
                if !credential.origins().contains(origin) && !elsewhere.contains(&serialized) {
                    elsewhere.push(serialized);
                }
            }
        }
        if !elsewhere.is_empty() {
            text.push_str(&format!(
                "data flow: data read as the account of {} can reach {}\n",
                escape(&request.id),
                elsewhere.join(", ")
            ));
        }
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
    if (!manifest.connectors().is_empty() || manifest.storage().is_some())
        && options.expect.is_none()
    {
        return usage_error(&format!(
            "approve needs --expect <digest> for a widget that declares connectors or storage: run `runtime permissions {}` and give the approval digest it prints",
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
    if let Some(storage) = manifest.storage() {
        println!("storage: quota {} bytes", storage.quota_bytes);
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
        for request in manifest
            .connectors()
            .iter()
            .filter(|request| request.account)
        {
            let access = checked
                .connectors()
                .iter()
                .find(|loaded| loaded.id() == request.id)
                .and_then(|loaded| loaded.connector().credential())
                .map(|credential| access_list(credential.access()))
                .unwrap_or_default();
            println!("account use: {} ({access})", escape(&request.id));
        }
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
    match (permission.as_str(), on) {
        // Turning storage off deletes nothing (specs/004-plugin-storage/spec.md, FR-011)
        ("storage", false) => println!("storage off (data kept)"),
        (permission, on) => println!("{} {}", escape(permission), if *on { "on" } else { "off" }),
    }
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
            // The store is the plugin's own state: removing the approval keeps it (FR-011)
            let has_store = runtime.list_stores().iter().any(|store| {
                store
                    .identity
                    .as_ref()
                    .is_some_and(|(store_id, _)| store_id == id.as_str())
            });
            if has_store {
                println!("storage kept: run runtime storage clear to delete it");
            }
        }
        Err(error) => {
            eprintln!("error: cannot start the runtime: {error:#}");
            return ExitCode::from(PLUGIN_FAILED);
        }
    }
    ExitCode::SUCCESS
}

/// Values longer than this are cut by `storage show`, with their size shown.
const SHOWN_ENTRY_BYTES: usize = 1024;

/// A runtime on the data directory of `options`, for the storage commands that need no package.
fn storage_runtime(options: &Options) -> Result<Runtime, ExitCode> {
    let data = data_dir(options).map_err(|message| usage_error(&message))?;
    Runtime::new(Limits::default(), RuntimeOptions::new(data)).map_err(|error| {
        eprintln!("error: cannot start the runtime: {error:#}");
        ExitCode::from(PLUGIN_FAILED)
    })
}

/// Lists every store in the data directory, sorted by id then origin, with the quota the grant
/// record approved for it, if any. Changes nothing.
fn storage_list(options: &Options) -> ExitCode {
    let runtime = match storage_runtime(options) {
        Ok(runtime) => runtime,
        Err(status) => return status,
    };
    let grants = match grant_store(options.grants.as_ref()) {
        Ok(grants) => grants,
        Err(message) => return usage_error(&message),
    };
    let stores = runtime.list_stores();
    if stores.is_empty() {
        println!("no stores");
    }
    for store in stores {
        let (name, origin) = match &store.identity {
            Some((id, origin)) => (escape(id), escape(origin)),
            None => (
                escape(&store.file.file_name().unwrap_or_default().to_string_lossy()),
                "-".to_owned(),
            ),
        };
        let state = match &store.state {
            runtime::storage::StoreState::Used(used) => {
                let (id, store_origin) = store.identity.clone().unwrap_or_default();
                let approved = PluginId::parse(&id)
                    .ok()
                    .and_then(|id| grants.entry(&id).ok().flatten())
                    .filter(|entry| entry.origin == store_origin)
                    .and_then(|entry| entry.storage_quota_bytes);
                let mut state = match approved {
                    Some(quota) => format!("used {used} of {quota} bytes"),
                    None => format!("used {used} bytes, not approved"),
                };
                if !Path::new(&store_origin).exists() {
                    state.push_str(", no file");
                }
                state
            }
            runtime::storage::StoreState::Damaged(detail) => format!("damaged: {}", escape(detail)),
            runtime::storage::StoreState::Unreadable(detail) => escape(detail),
        };
        println!("{name}  {origin}  {state}");
    }
    ExitCode::SUCCESS
}

/// Prints the store of a package, every name and value quoted and escaped, one entry per line.
/// Changes nothing and needs no approval.
fn storage_show(options: &Options) -> ExitCode {
    let target = match Target::open(options) {
        Ok(target) => target,
        Err(status) => return status,
    };
    let checked = match target.check() {
        Ok(checked) => checked,
        Err(status) => return status,
    };
    let manifest = checked.manifest();
    println!(
        "store: {} from {}",
        escape(manifest.id().as_str()),
        escape(&target.origin)
    );
    let view = match target.runtime.show_store(manifest.id(), &target.origin) {
        Ok(Some(view)) => view,
        Ok(None) => {
            println!("no store yet");
            return ExitCode::SUCCESS;
        }
        Err(error) => return refused(&error),
    };
    let quota = manifest.storage().map_or_else(String::new, |storage| {
        format!(" of {}", storage.quota_bytes)
    });
    println!(
        "used {}{quota} bytes, {} entries",
        view.used_bytes,
        view.entries.len()
    );
    for (name, value) in &view.entries {
        println!(
            "{} = {}",
            runtime::quote_entry(name, SHOWN_ENTRY_BYTES),
            runtime::quote_entry(value, SHOWN_ENTRY_BYTES)
        );
    }
    ExitCode::SUCCESS
}

/// Empties one store, by package or by id and origin; a store that is damaged, or whose package
/// file is gone, is removed instead. No other store changes.
fn storage_clear(options: &Options) -> ExitCode {
    let (runtime, id, origin) = match &options.store {
        Some((id, origin)) => match storage_runtime(options) {
            Ok(runtime) => (runtime, id.clone(), origin.clone()),
            Err(status) => return status,
        },
        None => {
            let target = match Target::open(options) {
                Ok(target) => target,
                Err(status) => return status,
            };
            let checked = match target.check() {
                Ok(checked) => checked,
                Err(status) => return status,
            };
            let id = checked.manifest().id().to_string();
            (target.runtime, id, target.origin)
        }
    };
    match runtime.clear_store(&id, &origin) {
        Ok(runtime::storage::Cleared::Emptied { used_bytes }) => println!(
            "cleared {} from {} ({used_bytes} bytes)",
            escape(&id),
            escape(&origin)
        ),
        Ok(runtime::storage::Cleared::Removed { reason }) => println!(
            "removed {} from {} ({})",
            escape(&id),
            escape(&origin),
            escape(&reason)
        ),
        Ok(runtime::storage::Cleared::NoStore) => {
            println!("no store of {} from {}", escape(&id), escape(&origin));
        }
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
        let runtime = development_runtime(options)?;
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

/// A runtime in development mode over `--data` and `--connectors`. On failure, prints why and
/// returns the exit status.
fn development_runtime(options: &Options) -> Result<Runtime, ExitCode> {
    let data_dir = data_dir(options).map_err(|message| usage_error(&message))?;
    // The development tool, the only place local connectors load (FR-018)
    let mut runtime_options = RuntimeOptions::new(data_dir).development(true);
    if let Some(connectors) = &options.connectors {
        runtime_options = runtime_options.connectors_dir(connectors);
    }
    Runtime::new(Limits::default(), runtime_options).map_err(|error| {
        eprintln!("error: cannot start the runtime: {error:#}");
        ExitCode::from(PLUGIN_FAILED)
    })
}

/// Where a connector comes from, as `accounts` prints it.
fn connector_from(origin: &str) -> String {
    if origin == "catalog" {
        "official catalog".to_owned()
    } else {
        format!("local file {}", escape(origin))
    }
}

/// `--origin` as the record names origins: the canonical path of a file that exists, or the
/// path as given when the file is gone.
fn given_origin(options: &Options) -> Option<String> {
    options.account_origin.as_ref().map(|given| {
        if given == "catalog" {
            return given.clone();
        }
        std::fs::canonicalize(given)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| given.clone())
    })
}

/// The `used by:` line: the widgets approved to use the account, when there are any.
fn used_by(options: &Options, connector: &str, origin: &str) -> Option<String> {
    let plugins = grant_store(options.grants.as_ref())
        .ok()?
        .using_account(connector, origin)
        .ok()?;
    let plugins: Vec<String> = plugins.iter().map(|plugin| escape(plugin)).collect();
    (!plugins.is_empty()).then(|| format!("used by: {}", plugins.join(", ")))
}

/// Connects an account or a key to a connector, in the development vault
/// (`specs/005-plugin-credentials/contracts/cli.md`). The sign-in's address is printed and opened
/// in the system browser; a key is read from the terminal without echo, never from an argument.
fn accounts_connect(options: &Options) -> ExitCode {
    let runtime = match development_runtime(options) {
        Ok(runtime) => runtime,
        Err(status) => return status,
    };
    let id = &options.plugin;
    let (connector, origin) = match runtime.find_connector(id) {
        Ok(found) => found,
        Err(error) => return refused(&error),
    };
    if let Some(given) = given_origin(options)
        && given != origin
    {
        return usage_error(&format!(
            "{} is found at {}, not at {}",
            escape(id),
            escape(&origin),
            escape(&given)
        ));
    }
    let Some(credential) = connector.credential() else {
        eprintln!(
            "error: invalid-connector: {} declares no credential",
            escape(id)
        );
        return ExitCode::from(PLUGIN_FAILED);
    };
    let development = if origin == "catalog" {
        ""
    } else {
        ", development"
    };
    let kind = match credential.kind() {
        CredentialKind::Account => "account",
        CredentialKind::Key => "key",
    };
    println!(
        "{kind} for {} ({}{development})",
        escape(id),
        connector_from(&origin)
    );
    if !credential.access().is_empty() {
        println!("asks for: {}", access_list(credential.access()));
    }
    let connected = match credential.kind() {
        CredentialKind::Account => {
            let minutes = Limits::default().sign_in_time_limit.as_secs().div_ceil(60);
            let no_browser = options.no_browser;
            let open = move |address: &url::Url| {
                // The address holds the state and the challenge, never the verifier
                let shown = escape(address.as_str());
                if no_browser {
                    println!("open {shown} in a browser");
                } else {
                    println!("opening {shown} in the browser");
                    if let Err(error) = open::that(address.as_str()) {
                        println!(
                            "the browser did not open ({}): open the address yourself",
                            escape(&error.to_string())
                        );
                    }
                }
                println!("waiting for the browser ({minutes} min)…");
            };
            runtime.connect_account(id, options.client.as_deref(), &open)
        }
        CredentialKind::Key => {
            use std::io::Write as _;
            let key = {
                // Without a terminal there is nothing to hide, and rpassword finds no key either
                #[cfg(unix)]
                let _echo_off = EchoOff::start().ok();
                eprint!("type the key (it is not shown): ");
                let _ = std::io::stderr().flush();
                zeroize::Zeroizing::new(rpassword::read_password().unwrap_or_default())
            };
            runtime.connect_key(id, &key)
        }
    };
    match connected {
        Ok(Connected::New(account)) => println!("connected: {}", account_label(&account)),
        Ok(Connected::Again(account)) => println!("reconnected: {}", account_label(&account)),
        Err(error) => return refused(&error),
    }
    if let Some(line) = used_by(options, id, &origin) {
        println!("{line}");
    }
    ExitCode::SUCCESS
}

/// The terminal's echo, off from before the key prompt is shown until dropped (R14). rpassword
/// turns it off only once it starts reading, after the prompt, so a key typed as soon as the
/// prompt appeared could be echoed. These are rpassword's own settings, applied with
/// `TCSAFLUSH`, which also discards anything typed before, which the terminal may have echoed,
/// as glibc's `getpass` does. rpassword then reads with them and leaves them as it found them.
#[cfg(unix)]
struct EchoOff {
    tty: std::fs::File,
    original: libc::termios,
}

#[cfg(unix)]
impl EchoOff {
    fn start() -> std::io::Result<EchoOff> {
        use std::os::fd::AsRawFd as _;
        let tty = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")?;
        let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
        // SAFETY: the descriptor is open for as long as `tty` lives, and tcgetattr writes a
        // whole termios into `original` when it returns 0
        if unsafe { libc::tcgetattr(tty.as_raw_fd(), original.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: tcgetattr returned 0, so it filled `original`
        let original = unsafe { original.assume_init() };
        let mut hidden = original;
        hidden.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ECHONL | libc::ISIG);
        hidden.c_cc[libc::VMIN] = 1;
        hidden.c_cc[libc::VTIME] = 0;
        // SAFETY: the descriptor is open, and `hidden` is a whole termios
        if unsafe { libc::tcsetattr(tty.as_raw_fd(), libc::TCSAFLUSH, &hidden) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(EchoOff { tty, original })
    }
}

#[cfg(unix)]
impl Drop for EchoOff {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd as _;
        // SAFETY: the descriptor is open until `tty` drops after this, and `original` is the
        // whole termios tcgetattr read
        unsafe { libc::tcsetattr(self.tty.as_raw_fd(), libc::TCSANOW, &self.original) };
    }
}

/// Every account in the development vault's record, sorted by connector id and origin; changes
/// nothing.
fn accounts_list(options: &Options) -> ExitCode {
    let runtime = match development_runtime(options) {
        Ok(runtime) => runtime,
        Err(status) => return status,
    };
    let mut accounts = match runtime.list_accounts() {
        Ok(accounts) => accounts,
        Err(error) => return refused(&error),
    };
    if accounts.is_empty() {
        println!("nothing connected");
        return ExitCode::SUCCESS;
    }
    accounts.sort_by(|a, b| (&a.connector, &a.origin).cmp(&(&b.connector, &b.origin)));
    for account in &accounts {
        let gone = if account.origin_exists() {
            ""
        } else {
            " (no file)"
        };
        println!(
            "{}  {}{gone}",
            escape(&account.connector),
            connector_from(&account.origin)
        );
        // The connector as found now, when it is still the one at the account's origin
        let current = runtime
            .find_connector(&account.connector)
            .ok()
            .filter(|(_, origin)| *origin == account.origin)
            .and_then(|(connector, _)| {
                connector
                    .credential()
                    .map(|credential| credential.access().to_vec())
            });
        let state = match &account.needs_connecting {
            Some(needs) => {
                let why = match needs.reason {
                    Reason::Refused => "the service refused it".to_owned(),
                    Reason::RefreshFailed => "it could not be refreshed".to_owned(),
                    Reason::MissingFromKeychain => "it is missing from the keychain".to_owned(),
                    Reason::LessAccess => format!(
                        "the connector asks for more access than it holds ({})",
                        current.as_deref().map(access_list).unwrap_or_default()
                    ),
                };
                format!("needs connecting again: {why}")
            }
            None => {
                let mut state = format!(
                    "connected {}, refreshed {}",
                    minute(account.connected_at_ms),
                    minute(account.refreshed_at_ms)
                );
                if let Some(expires) = account.expires_at_ms {
                    state.push_str(&format!(", expires {}", minute(expires)));
                }
                state
            }
        };
        println!("  account: {}, {state}", account_label(account));
        if account.kind == AccountKind::Account || !account.access_asked.is_empty() {
            let remark = match &current {
                Some(now) if account.access_asked.iter().any(|item| !now.contains(item)) => {
                    " (more than the connector asks for now: connect again to narrow it)"
                }
                _ => " (as asked)",
            };
            println!("  access: {}{remark}", access_list(&account.access_asked));
        }
        if let Some(client) = &account.client {
            let from = if client.from == "option" {
                "from --client"
            } else {
                "built in"
            };
            println!("  client: {} ({from})", escape(&client.client_id));
        }
        if let Some(line) = used_by(options, &account.connector, &account.origin) {
            println!("  {line}");
        }
    }
    ExitCode::SUCCESS
}

/// `2026-10-25 14:02 UTC`.
fn minute(at_ms: u64) -> String {
    let time = utc(at_ms);
    format!("{} {} UTC", &time[..10], &time[11..16])
}

/// Disconnects a connector's account: deletes its credential from the keychain, the account from
/// the record and the kept data read as it, then asks the service to revoke it.
fn accounts_disconnect(options: &Options) -> ExitCode {
    let runtime = match development_runtime(options) {
        Ok(runtime) => runtime,
        Err(status) => return status,
    };
    let id = &options.plugin;
    let origin = given_origin(options);
    // Where the service revokes, from the connector as found now, for the output
    let revoke_host = runtime
        .find_connector(id)
        .ok()
        .and_then(|(connector, _)| {
            connector
                .credential()
                .and_then(|credential| credential.account())
                .and_then(|account| account.revoke())
                .and_then(|revoke| revoke.host_str().map(str::to_owned))
        })
        .unwrap_or_else(|| "the service".to_owned());
    let gone = match runtime.disconnect_account(id, origin.as_deref()) {
        Ok(Some(gone)) => gone,
        Ok(None) => {
            eprintln!(
                "error: account-not-connected: no account is connected to {}",
                escape(id)
            );
            return ExitCode::from(PLUGIN_FAILED);
        }
        Err(error) => return refused(&error),
    };
    println!(
        "disconnected {}: {}",
        escape(id),
        account_label(&gone.account)
    );
    println!("deleted from the keychain");
    let revoke_at = match &gone.revoke_page {
        Some(page) => format!("revoke it at {}", escape(page)),
        None => "revoke it at the service".to_owned(),
    };
    match &gone.revoked {
        Some(Ok(())) => println!("revoked at {}", escape(&revoke_host)),
        Some(Err(why)) => println!("revocation failed: {}; {revoke_at}", escape(why)),
        None => println!("the service offers no revocation: {revoke_at}"),
    }
    let discarded: Vec<String> = gone
        .kept_discarded
        .iter()
        .map(|plugin| escape(plugin))
        .collect();
    if discarded.is_empty() {
        println!("kept data discarded: none");
    } else {
        println!("kept data discarded: {}", discarded.join(", "));
    }
    ExitCode::SUCCESS
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
