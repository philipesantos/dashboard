//! The docs runner (`specs/006-contract-v1-docs/research.md`, R8, and
//! `specs/006-contract-v1-docs/contracts/docs-format.md`): runs every block marked `check` in
//! the pages under `docs/authors/`, as written, one session per page, and compares each
//! command's whole output with the page. `every_page_runs_as_written` is ignored in `cargo test`
//! because it builds and installs the CLI; the docs job runs it with `--ignored`. The other tests
//! check the runner itself, on Markdown in memory.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use regex::Regex;
use tempfile::TempDir;

/// The address the walkthrough clones; git's `insteadOf` sends it to this checkout.
const REPOSITORY: &str = "https://github.com/philipesantos/dashboard";

/// Outside CI, the runner starts only with this set, since it clears the development data.
const OPT_IN: &str = "DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA";

/// The target folder `cargo install` builds in, when the docs job gives its cached one.
const TARGET_DIR: &str = "DASHBOARD_DOCS_TARGET_DIR";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum System {
    Linux,
    Macos,
    Windows,
}

impl System {
    const ALL: [System; 3] = [System::Linux, System::Macos, System::Windows];

    fn current() -> System {
        if cfg!(windows) {
            System::Windows
        } else if cfg!(target_os = "macos") {
            System::Macos
        } else {
            System::Linux
        }
    }

    fn parse(name: &str) -> Result<System, String> {
        match name {
            "linux" => Ok(System::Linux),
            "macos" => Ok(System::Macos),
            "windows" => Ok(System::Windows),
            other => Err(format!("unknown system {other:?} in os=")),
        }
    }
}

/// A page's runnable content, in order.
#[derive(Debug, PartialEq)]
struct Page {
    /// The test services of its `<!-- check: service=… -->` comment.
    services: Vec<String>,
    blocks: Vec<Block>,
}

#[derive(Debug, PartialEq)]
struct Block {
    /// The line of the block's opening fence, counting from 1.
    line: usize,
    systems: Vec<System>,
    kind: BlockKind,
}

#[derive(Debug, PartialEq)]
enum BlockKind {
    Commands {
        exit: i32,
        /// `output=any`: only the exit code is checked.
        any_output: bool,
        steps: Vec<Step>,
    },
    File {
        path: String,
        content: String,
    },
}

/// One command and the lines it must print.
#[derive(Debug, PartialEq)]
struct Step {
    line: usize,
    text: String,
    expected: Vec<String>,
}

/// Reads the blocks marked `check` of a page, refusing what the format does not allow.
fn parse_page(markdown: &str) -> Result<Page, String> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut services = None;
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some(rest) = trimmed.strip_prefix("<!-- check:") {
            let Some(rest) = rest.trim_end().strip_suffix("-->") else {
                return Err(format!(
                    "line {}: the check comment does not end",
                    index + 1
                ));
            };
            if services.is_some() {
                return Err(format!("line {}: a second check comment", index + 1));
            }
            services = Some(
                parse_services(rest.trim())
                    .map_err(|error| format!("line {}: {error}", index + 1))?,
            );
            index += 1;
            continue;
        }
        let ticks = trimmed.chars().take_while(|&c| c == '`').count();
        if ticks < 3 {
            index += 1;
            continue;
        }
        let info = trimmed[ticks..].trim();
        let opening = index + 1;
        let mut content = Vec::new();
        index += 1;
        loop {
            let Some(line) = lines.get(index) else {
                return Err(format!("line {opening}: the block does not end"));
            };
            index += 1;
            let inner = line.trim();
            if inner.len() >= ticks && inner.chars().all(|c| c == '`') {
                break;
            }
            let strip = line.len() - line.trim_start().len();
            content.push(&line[strip.min(indent)..]);
        }
        let words: Vec<&str> = info.split_whitespace().collect();
        if !words.contains(&"check") {
            continue;
        }
        let block = parse_block(&words, opening, &content)
            .map_err(|error| format!("line {opening}: {error}"))?;
        blocks.push(block);
    }
    Ok(Page {
        services: services.unwrap_or_default(),
        blocks,
    })
}

fn parse_services(text: &str) -> Result<Vec<String>, String> {
    let Some(names) = text.strip_prefix("service=") else {
        return Err(format!("unknown check comment {text:?}"));
    };
    let names: Vec<String> = names.split(',').map(str::to_owned).collect();
    for name in &names {
        if !matches!(name.as_str(), "github" | "notes") {
            return Err(format!("unknown test service {name:?}"));
        }
    }
    Ok(names)
}

fn parse_block(words: &[&str], opening: usize, content: &[&str]) -> Result<Block, String> {
    let language = words[0];
    if language == "check" {
        return Err("a checked block names its language first".to_owned());
    }
    let mut systems = None;
    let mut exit = None;
    let mut file = None;
    let mut any_output = false;
    for word in &words[1..] {
        if *word == "check" {
            continue;
        }
        if let Some(names) = word.strip_prefix("os=") {
            let parsed: Result<Vec<System>, String> = names.split(',').map(System::parse).collect();
            systems = Some(parsed?);
        } else if let Some(code) = word.strip_prefix("exit=") {
            exit = Some(
                code.parse()
                    .map_err(|_| format!("exit={code} is not a number"))?,
            );
        } else if let Some(path) = word.strip_prefix("file=") {
            file = Some(path.to_owned());
        } else if *word == "output=any" {
            any_output = true;
        } else if word.starts_with("service=") {
            return Err(
                "services are declared in the page's check comment, never on a block".to_owned(),
            );
        } else {
            return Err(format!("unknown option {word:?}"));
        }
    }
    if language != "console" {
        let Some(path) = file else {
            return Err(format!("a checked {language} block needs file=<path>"));
        };
        if exit.is_some() || any_output {
            return Err("exit= and output=any are for console blocks".to_owned());
        }
        return Ok(Block {
            line: opening,
            systems: systems.unwrap_or_else(|| System::ALL.to_vec()),
            kind: BlockKind::File {
                path,
                content: content.iter().map(|line| format!("{line}\n")).collect(),
            },
        });
    }
    if file.is_some() {
        return Err("file= is for file blocks, not console blocks".to_owned());
    }
    let steps = parse_steps(opening, content)?;
    let powershell = steps.iter().any(|step| step.text.starts_with("PS> "));
    let bash = steps.iter().any(|step| step.text.starts_with("$ "));
    if powershell && bash {
        return Err(
            "the block mixes $ and PS> prompts; give each form a block with os=".to_owned(),
        );
    }
    let systems = systems.unwrap_or_else(|| System::ALL.to_vec());
    if powershell && systems != [System::Windows] {
        return Err("PS> commands run only on Windows: say os=windows".to_owned());
    }
    if any_output && steps.iter().any(|step| runs_runtime(&step.text)) {
        return Err("output=any is refused on a block with a runtime command".to_owned());
    }
    let steps = steps
        .into_iter()
        .map(|step| Step {
            text: step.text[step.text.find(' ').unwrap_or(0) + 1..].to_owned(),
            ..step
        })
        .collect();
    Ok(Block {
        line: opening,
        systems,
        kind: BlockKind::Commands {
            exit: exit.unwrap_or(0),
            any_output,
            steps,
        },
    })
}

/// The commands of a console block, each with its prompt still on, and the lines after each.
fn parse_steps(opening: usize, content: &[&str]) -> Result<Vec<Step>, String> {
    let mut steps: Vec<Step> = Vec::new();
    let mut continued = false;
    for (offset, line) in content.iter().enumerate() {
        if continued {
            let step = steps
                .last_mut()
                .expect("a continued line follows a command");
            step.text.push('\n');
            step.text.push_str(line);
            continued = line.ends_with(" \\") || line.ends_with(" `");
            continue;
        }
        if line.starts_with("$ ") || line.starts_with("PS> ") {
            steps.push(Step {
                line: opening + 1 + offset,
                text: (*line).to_owned(),
                expected: Vec::new(),
            });
            continued = line.ends_with(" \\") || line.ends_with(" `");
        } else if let Some(step) = steps.last_mut() {
            step.expected.push(line.trim_end().to_owned());
        } else {
            return Err("output before the first command".to_owned());
        }
    }
    if steps.is_empty() {
        return Err("a checked console block has no command".to_owned());
    }
    for step in &mut steps {
        while step.expected.last().is_some_and(String::is_empty) {
            step.expected.pop();
        }
    }
    Ok(steps)
}

/// Whether a command runs the development CLI, whose output is always compared whole.
fn runs_runtime(text: &str) -> bool {
    let command = text[text.find(' ').unwrap_or(0)..].trim_start();
    command == "runtime" || command.starts_with("runtime ")
}

/// The pattern for one expected line: placeholders match only their kind, anything else is
/// literal. `<path>` captures, so its value can be checked against the session's folders.
fn line_pattern(expected: &str) -> Regex {
    let placeholder = Regex::new(r"<(digest|path|n|time|version)>").expect("a valid pattern");
    let mut pattern = String::from("^");
    let mut last = 0;
    for found in placeholder.captures_iter(expected) {
        let whole = found.get(0).expect("the match");
        pattern.push_str(&regex::escape(&expected[last..whole.start()]));
        pattern.push_str(match &found[1] {
            "digest" => "[0-9a-f]{64}",
            "path" => "(.+?)",
            "n" => "[0-9]+",
            "time" => {
                r"(?:[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z|[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2} UTC|[0-9]+ms)"
            }
            _ => r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?",
        });
        last = whole.end();
    }
    pattern.push_str(&regex::escape(&expected[last..]));
    pattern.push('$');
    Regex::new(&pattern).expect("an escaped pattern is valid")
}

/// Whether `actual` is what `expected` shows; every `<path>` must lie under one of `roots`.
fn line_matches(expected: &str, actual: &str, roots: &[String]) -> bool {
    let Some(found) = line_pattern(expected).captures(actual) else {
        return false;
    };
    found.iter().skip(1).flatten().all(|path| {
        let path = normalize(path.as_str());
        roots.iter().any(|root| path.starts_with(&normalize(root)))
    })
}

fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path
    }
}

/// Compares a command's whole output with the page; `None` when they agree.
fn compare(expected: &[String], actual: &[String], roots: &[String]) -> Option<String> {
    let agree = expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .all(|(expected, actual)| line_matches(expected, actual, roots));
    if agree {
        return None;
    }
    Some(format!(
        "expected:\n{}\nactual:\n{}",
        indent(expected),
        indent(actual)
    ))
}

fn indent(lines: &[String]) -> String {
    if lines.is_empty() {
        return "    (nothing)".to_owned();
    }
    lines
        .iter()
        .map(|line| format!("    {line}\n"))
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// Refuses to start outside CI unless the person said the development data may be cleared.
fn guard(ci: Option<&str>, opt_in: Option<&str>) -> Result<(), String> {
    if ci.is_some_and(|value| !value.is_empty()) || opt_in == Some("1") {
        Ok(())
    } else {
        Err(format!(
            "the docs runner clears the development data directory and grant record before each \
             page; outside CI, set {OPT_IN}=1 to run it"
        ))
    }
}

fn executable(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// `PATH` with the session's folders first and every other folder that holds a `runtime`
/// removed, so a page never runs a CLI from outside the session.
fn session_path(first: &[PathBuf], path: &OsString) -> OsString {
    let runtime = executable("runtime");
    let kept = std::env::split_paths(path).filter(|dir| !dir.join(&runtime).is_file());
    std::env::join_paths(first.iter().cloned().chain(kept)).expect("PATH entries join")
}

/// What a command's environment adds for it alone: the target folder and the release profile
/// without LTO for `cargo install`, never for the author's own `cargo build` (research R3).
fn command_env(text: &str, target_dir: &Path) -> Vec<(String, String)> {
    if text.trim_start().starts_with("cargo install") {
        vec![
            (
                "CARGO_TARGET_DIR".to_owned(),
                target_dir.display().to_string(),
            ),
            ("CARGO_PROFILE_RELEASE_LTO".to_owned(), "false".to_owned()),
            (
                "CARGO_PROFILE_RELEASE_CODEGEN_UNITS".to_owned(),
                "16".to_owned(),
            ),
        ]
    } else {
        Vec::new()
    }
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("runtime/ is in the repository")
        .to_path_buf()
}

/// One page's session: its folder, where the next command runs, and its environment.
struct Session {
    folder: TempDir,
    scratch: TempDir,
    cwd: PathBuf,
    env: Vec<(String, String)>,
    path: OsString,
    roots: Vec<String>,
    target_dir: PathBuf,
}

impl Session {
    fn start() -> Session {
        let folder = TempDir::new().expect("a session folder");
        let scratch = TempDir::new().expect("a folder for the runner's own files");
        let session = folder.path().to_path_buf();
        let mut env: Vec<(String, String)> = std::env::vars()
            .filter(|(name, _)| {
                // Cargo's variables for this test must not reach the commands' own cargo
                (!name.starts_with("CARGO_") || name == "CARGO_HOME")
                    && name != "CARGO"
                    && name != "DASHBOARD_TEST_NETWORK"
                    && name != "PATH"
            })
            .collect();
        let base = repository().display().to_string().replace('\\', "/");
        env.extend([
            ("GIT_CONFIG_COUNT".to_owned(), "1".to_owned()),
            (
                "GIT_CONFIG_KEY_0".to_owned(),
                format!("url.{base}.insteadOf"),
            ),
            ("GIT_CONFIG_VALUE_0".to_owned(), REPOSITORY.to_owned()),
            (
                "CARGO_INSTALL_ROOT".to_owned(),
                session.join("install").display().to_string(),
            ),
        ]);
        let path = session_path(
            &[session.join("bin"), session.join("install").join("bin")],
            &std::env::var_os("PATH").unwrap_or_default(),
        );
        let mut roots = vec![session.display().to_string()];
        if let Ok(canonical) = session.canonicalize() {
            roots.push(canonical.display().to_string());
        }
        if let Some(data) = runtime::RuntimeOptions::default_development_data_dir()
            .and_then(|dir| dir.parent().map(Path::to_path_buf))
        {
            roots.push(data.display().to_string());
        }
        let target_dir = std::env::var_os(TARGET_DIR)
            .map(PathBuf::from)
            .unwrap_or_else(|| repository().join("target"));
        Session {
            cwd: session,
            folder,
            scratch,
            env,
            path,
            roots,
            target_dir,
        }
    }

    fn folder(&self) -> &Path {
        self.folder.path()
    }

    /// Runs one command as the page shows it and returns its exit code and the lines it printed,
    /// stdout and stderr merged. `cd <dir>` alone changes where the next commands run.
    fn run(&mut self, text: &str) -> (i32, Vec<String>) {
        if let Some(dir) = text.strip_prefix("cd ").filter(|dir| !dir.contains(' ')) {
            let next = self.cwd.join(dir);
            if next.is_dir() {
                self.cwd = next;
                return (0, Vec::new());
            }
            return (1, vec![format!("cd: {dir}: no such folder")]);
        }
        let output = self.scratch.path().join("output");
        let file = fs::File::create(&output).expect("the output file");
        let mut command = if cfg!(windows) {
            let mut command = Command::new("pwsh");
            command.args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "$ErrorActionPreference = 'Stop'; {text}; if ($null -ne $LASTEXITCODE) {{ exit $LASTEXITCODE }}"
                ),
            ]);
            command
        } else {
            let mut command = Command::new("bash");
            command.args(["-c", text]);
            command
        };
        command
            .current_dir(&self.cwd)
            .env_clear()
            .envs(self.env.iter().cloned())
            .envs(command_env(text, &self.target_dir))
            .env("PATH", &self.path)
            .stdin(Stdio::null())
            .stdout(file.try_clone().expect("the output file is shared"))
            .stderr(file);
        let status = command.status().expect("the shell starts");
        let printed = fs::read(&output).expect("the output file is read");
        let printed = String::from_utf8_lossy(&printed).replace("\r\n", "\n");
        let mut lines: Vec<String> = printed
            .lines()
            .map(|line| line.trim_end().to_owned())
            .collect();
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        (status.code().unwrap_or(-1), lines)
    }
}

/// The development data directory and grant record, which every page starts without.
fn clear_development_records() {
    if let Some(data) = runtime::RuntimeOptions::default_development_data_dir() {
        match fs::remove_dir_all(&data) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("cannot clear {}: {error}", data.display()),
        }
    }
    if let Some(grants) = runtime::grants::GrantStore::default_development_path() {
        // The record and its lock file, `<record>.lock`
        let mut lock = grants.clone().into_os_string();
        lock.push(".lock");
        for file in [grants, PathBuf::from(lock)] {
            match fs::remove_file(&file) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("cannot clear {}: {error}", file.display()),
            }
        }
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the copy's folder");
    for entry in fs::read_dir(from).expect("the folder is read") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("its type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("the file is copied");
        }
    }
}

/// Where every page but the walkthrough starts: a local clone, the CLI this test build made
/// first on `PATH`, and for reference pages a copy of the walkthrough's crate
/// (`contracts/docs-format.md`).
fn set_up(session: &mut Session, page: &str) {
    let folder = session.folder().to_path_buf();
    let (code, lines) = session.run(&format!("git clone --quiet {REPOSITORY}"));
    assert_eq!(code, 0, "the session's clone failed:\n{}", lines.join("\n"));
    let bin = folder.join("bin");
    fs::create_dir_all(&bin).expect("the session's bin folder");
    fs::copy(
        env!("CARGO_BIN_EXE_runtime"),
        bin.join(executable("runtime")),
    )
    .expect("the CLI is copied into the session");
    if page.starts_with("reference/") {
        let crate_dir = folder.join("my-widget");
        copy_tree(
            &repository().join("docs/authors/getting-started"),
            &crate_dir,
        );
        fs::create_dir_all(crate_dir.join("wit")).expect("the crate's wit folder");
        fs::copy(
            repository().join("wit/history/1.3.0/widget.wit"),
            crate_dir.join("wit/widget.wit"),
        )
        .expect("the contract is copied");
    }
}

/// The crate a page builds, kept beside it as a folder of the same name
/// (`docs/authors/getting-started/` for `getting-started.md`): each of its files is a file block
/// of the page with the same content, and the page builds with its `Cargo.lock`, which
/// `cargo audit` and `cargo deny` check (research R8). Other file blocks, such as a broken
/// manifest the page shows on purpose, are the page's own.
struct Kept {
    dir: PathBuf,
    /// Where the page's `Cargo.toml` went, so its `Cargo.lock` beside it.
    locks: Vec<PathBuf>,
    /// The kept files the page wrote so far.
    written: Vec<String>,
}

impl Kept {
    fn of(page: &Path) -> Option<Kept> {
        let dir = page.with_extension("");
        dir.is_dir().then_some(Kept {
            dir,
            locks: Vec::new(),
            written: Vec::new(),
        })
    }

    /// The kept files, by their path in the crate, `Cargo.lock` aside.
    fn files(&self) -> Vec<String> {
        fn walk(dir: &Path, root: &Path, found: &mut Vec<String>) {
            for entry in fs::read_dir(dir).expect("the kept crate is read") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    walk(&path, root, found);
                } else {
                    let relative = path.strip_prefix(root).expect("under the crate");
                    found.push(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut found = Vec::new();
        walk(&self.dir, &self.dir, &mut found);
        found.retain(|path| path != "Cargo.lock" && !path.starts_with("target/"));
        found.sort();
        found
    }

    /// A file block of a kept file's path has its content.
    fn check_file(&mut self, path: &str, content: &str) -> Result<(), String> {
        let kept = self.dir.join(path);
        let Ok(kept_text) = fs::read_to_string(&kept) else {
            return Ok(());
        };
        self.written.push(path.to_owned());
        if kept_text.replace("\r\n", "\n") == content {
            Ok(())
        } else {
            Err(format!(
                "{path} differs from {}; the page and the kept crate change together",
                kept.display()
            ))
        }
    }

    /// Puts the kept `Cargo.lock` beside a `Cargo.toml` the page wrote.
    fn seed_lock(&mut self, written: &Path) {
        if written.file_name().is_some_and(|name| name == "Cargo.toml") {
            let lock = written.with_file_name("Cargo.lock");
            fs::copy(self.dir.join("Cargo.lock"), &lock).expect("the kept Cargo.lock is copied");
            self.locks.push(lock);
        }
    }

    /// Every kept file was a file block of the page, and the page built with the kept
    /// dependencies: cargo changed no `Cargo.lock` it was given.
    fn check_end(&self) -> Result<(), String> {
        for path in self.files() {
            if !self.written.contains(&path) {
                return Err(format!(
                    "{} is not a file block of the page",
                    self.dir.join(&path).display()
                ));
            }
        }
        let kept = fs::read(self.dir.join("Cargo.lock")).expect("the kept Cargo.lock is read");
        for lock in &self.locks {
            if fs::read(lock).ok().as_ref() != Some(&kept) {
                return Err(format!(
                    "cargo changed {}: update {} with the page",
                    lock.display(),
                    self.dir.join("Cargo.lock").display()
                ));
            }
        }
        Ok(())
    }
}

/// Runs one page's blocks in order and returns the first difference.
fn run_page(relative: &str, path: &Path, page: &Page) -> Result<(), String> {
    let system = System::current();
    clear_development_records();
    let mut session = Session::start();
    if relative != "getting-started.md" {
        set_up(&mut session, relative);
    }
    let mut kept = Kept::of(path);
    for block in page
        .blocks
        .iter()
        .filter(|block| block.systems.contains(&system))
    {
        match &block.kind {
            BlockKind::File { path, content } => {
                if let Some(kept) = &mut kept {
                    kept.check_file(path, content)
                        .map_err(|error| format!("line {}: {error}", block.line))?;
                }
                let target = session.cwd.join(path);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).expect("the file's folder");
                }
                fs::write(&target, content).expect("the file is written");
                if let Some(kept) = &mut kept {
                    kept.seed_lock(&target);
                }
            }
            BlockKind::Commands {
                exit,
                any_output,
                steps,
            } => {
                for step in steps {
                    let (code, printed) = session.run(&step.text);
                    let differs = if *any_output {
                        None
                    } else {
                        compare(&step.expected, &printed, &session.roots)
                    };
                    if code != *exit || differs.is_some() {
                        return Err(format!(
                            "line {}: {}\nexit code {code}, expected {exit}\n{}",
                            step.line,
                            step.text,
                            differs.unwrap_or_else(|| format!("output:\n{}", indent(&printed)))
                        ));
                    }
                }
            }
        }
    }
    match &kept {
        Some(kept) => kept.check_end(),
        None => Ok(()),
    }
}

fn pages() -> Vec<(String, PathBuf)> {
    fn walk(dir: &Path, root: &Path, found: &mut Vec<(String, PathBuf)>) {
        for entry in fs::read_dir(dir).expect("docs/authors/ is read") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, root, found);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                let relative = path.strip_prefix(root).expect("under the root");
                let relative = relative.to_string_lossy().replace('\\', "/");
                found.push((relative, path));
            }
        }
    }
    let root = repository().join("docs/authors");
    let mut found = Vec::new();
    walk(&root, &root, &mut found);
    found.sort();
    found
}

#[test]
#[ignore = "builds and installs the CLI; the docs job runs it with --ignored"]
fn every_page_runs_as_written() {
    if let Err(message) = guard(
        std::env::var("CI").ok().as_deref(),
        std::env::var(OPT_IN).ok().as_deref(),
    ) {
        panic!("{message}");
    }
    let mut failures = Vec::new();
    for (relative, path) in pages() {
        let markdown = fs::read_to_string(&path).expect("the page is read");
        let page = match parse_page(&markdown) {
            Ok(page) => page,
            Err(error) => {
                failures.push(format!("docs/authors/{relative}: {error}"));
                continue;
            }
        };
        if page.blocks.is_empty() {
            continue;
        }
        let walked = run_page(&relative, &path, &page);
        // Nothing of a page stays in the development records once it ends
        clear_development_records();
        if let Err(error) = walked {
            failures.push(format!("docs/authors/{relative}, {error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

// The runner's own checks, on Markdown in memory

fn commands(block: &Block) -> &[Step] {
    match &block.kind {
        BlockKind::Commands { steps, .. } => steps,
        BlockKind::File { .. } => panic!("a console block"),
    }
}

#[test]
fn reads_the_options_of_a_checked_block() {
    let page = parse_page(
        "Text\n\n```console check os=linux,macos exit=2\n$ runtime run x.wasm\nerror: not-granted\n```\n\n\
         ```toml file=Cargo.toml check\n[package]\nname = \"my-widget\"\n```\n\n\
         ```console check output=any\n$ cargo build --release\n```\n",
    )
    .expect("the page is read");
    assert_eq!(page.blocks.len(), 3);
    assert_eq!(page.blocks[0].line, 3);
    assert_eq!(page.blocks[0].systems, [System::Linux, System::Macos]);
    assert_eq!(
        page.blocks[0].kind,
        BlockKind::Commands {
            exit: 2,
            any_output: false,
            steps: vec![Step {
                line: 4,
                text: "runtime run x.wasm".to_owned(),
                expected: vec!["error: not-granted".to_owned()],
            }],
        }
    );
    assert_eq!(
        page.blocks[1].kind,
        BlockKind::File {
            path: "Cargo.toml".to_owned(),
            content: "[package]\nname = \"my-widget\"\n".to_owned(),
        }
    );
    assert_eq!(page.blocks[1].systems, System::ALL);
    assert!(matches!(
        page.blocks[2].kind,
        BlockKind::Commands {
            any_output: true,
            ..
        }
    ));
}

#[test]
fn a_block_without_check_is_not_run() {
    let page = parse_page("```console\n$ rm -rf /\n```\n\n```json\n{}\n```\n").expect("read");
    assert!(page.blocks.is_empty());
}

#[test]
fn powershell_runs_only_on_windows_and_never_beside_bash() {
    let windows =
        parse_page("```console check os=windows\nPS> Copy-Item a b\n```\n").expect("read");
    assert_eq!(windows.blocks[0].systems, [System::Windows]);
    assert_eq!(commands(&windows.blocks[0])[0].text, "Copy-Item a b");
    let everywhere = parse_page("```console check\nPS> Copy-Item a b\n```\n").unwrap_err();
    assert!(everywhere.contains("os=windows"), "{everywhere}");
    let mixed =
        parse_page("```console check os=windows\n$ cp a b\nPS> Copy-Item a b\n```\n").unwrap_err();
    assert!(mixed.contains("mixes"), "{mixed}");
}

#[test]
fn each_command_expects_the_lines_up_to_the_next_prompt() {
    let page = parse_page(
        "```console check\n$ runtime pack a --manifest m --output b   \npacked b\ndigest: <digest>\n\n$ ls\nb\n```\n",
    )
    .expect("read");
    let steps = commands(&page.blocks[0]);
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].text, "runtime pack a --manifest m --output b   ");
    assert_eq!(steps[0].expected, ["packed b", "digest: <digest>"]);
    assert_eq!(steps[1].expected, ["b"]);
    assert!(parse_page("```console check\nhello\n$ ls\n```\n").is_err());
}

#[test]
fn a_command_can_continue_on_the_next_line() {
    let page = parse_page("```console check\n$ runtime pack a \\\n  --output b\npacked b\n```\n")
        .expect("read");
    let steps = commands(&page.blocks[0]);
    assert_eq!(steps[0].text, "runtime pack a \\\n  --output b");
    assert_eq!(steps[0].expected, ["packed b"]);
}

#[test]
fn indented_blocks_in_lists_are_read() {
    let page =
        parse_page("1. Build:\n\n   ```console check output=any\n   $ cargo build\n   ```\n")
            .expect("read");
    assert_eq!(commands(&page.blocks[0])[0].text, "cargo build");
}

#[test]
fn each_placeholder_matches_only_its_kind() {
    let roots = ["/tmp/session".to_owned()];
    let digest = "a".repeat(64);
    assert!(line_matches(
        "digest: <digest>",
        &format!("digest: {digest}"),
        &roots
    ));
    assert!(!line_matches(
        "digest: <digest>",
        &format!("digest: {}", "a".repeat(63)),
        &roots
    ));
    assert!(!line_matches(
        "digest: <digest>",
        &format!("digest: {}", "A".repeat(64)),
        &roots
    ));
    assert!(line_matches("<n> requests", "12 requests", &roots));
    assert!(!line_matches("<n> requests", "twelve requests", &roots));
    assert!(line_matches(
        "packed <path>",
        "packed /tmp/session/my-widget.wasm",
        &roots
    ));
    assert!(!line_matches(
        "packed <path>",
        "packed /home/someone/my-widget.wasm",
        &roots
    ));
    assert!(line_matches(
        "at <time>",
        "at 2026-10-25T12:34:56.789Z",
        &roots
    ));
    assert!(line_matches(
        "since <time>",
        "since 2026-10-25 12:34 UTC",
        &roots
    ));
    assert!(line_matches("200 41B <time>", "200 41B 12ms", &roots));
    assert!(!line_matches("at <time>", "at yesterday", &roots));
    assert!(line_matches("version <version>", "version 1.3.0", &roots));
    assert!(line_matches(
        "version <version>",
        "version 1.0.0-beta.1",
        &roots
    ));
    assert!(!line_matches("version <version>", "version 1.3", &roots));
    assert!(line_matches("<id> is literal", "<id> is literal", &roots));
    assert!(!line_matches("<id> is literal", "x is literal", &roots));
    assert!(
        !line_matches("a.b", "aXb", &roots),
        "the rest is literal, not a pattern"
    );
}

#[test]
fn output_any_is_refused_on_a_runtime_command() {
    let error = parse_page("```console check output=any\n$ runtime run x.wasm\n```\n").unwrap_err();
    assert!(error.contains("output=any"), "{error}");
    assert!(parse_page("```console check output=any\n$ git clone x\n```\n").is_ok());
}

#[test]
fn services_are_declared_on_the_page_only() {
    let page = parse_page("<!-- check: service=github,notes -->\n\n# Connectors\n").expect("read");
    assert_eq!(page.services, ["github", "notes"]);
    let on_block =
        parse_page("```console check service=github\n$ runtime update x\n```\n").unwrap_err();
    assert!(on_block.contains("check comment"), "{on_block}");
    assert!(parse_page("<!-- check: service=gitlab -->\n").is_err());
}

#[test]
fn a_differing_line_shows_both_outputs() {
    let roots = ["/tmp".to_owned()];
    let expected = ["packed b".to_owned()];
    assert_eq!(compare(&expected, &["packed b".to_owned()], &roots), None);
    let differs = compare(&expected, &["packed c".to_owned()], &roots).expect("they differ");
    assert!(differs.contains("expected:\n    packed b"), "{differs}");
    assert!(differs.contains("actual:\n    packed c"), "{differs}");
    assert!(
        compare(&expected, &[], &roots).is_some(),
        "a missing line differs"
    );
}

#[test]
fn the_runner_starts_only_in_ci_or_when_asked() {
    assert!(guard(Some("true"), None).is_ok());
    assert!(guard(None, Some("1")).is_ok());
    let refused = guard(None, None).unwrap_err();
    assert!(refused.contains(OPT_IN), "{refused}");
    assert!(guard(Some(""), Some("yes")).is_err());
}

#[test]
fn a_page_never_runs_a_runtime_from_outside_the_session() {
    let other = TempDir::new().expect("a folder");
    fs::write(other.path().join(executable("runtime")), "").expect("a stray CLI");
    let plain = TempDir::new().expect("a folder");
    let path = std::env::join_paths([other.path(), plain.path()]).expect("a PATH");
    let session = PathBuf::from("/session/bin");
    let kept: Vec<PathBuf> =
        std::env::split_paths(&session_path(std::slice::from_ref(&session), &path)).collect();
    assert_eq!(kept, [session, plain.path().to_path_buf()]);
}

#[test]
fn only_cargo_install_gets_the_target_folder_and_the_profile() {
    let target = Path::new("/cache/target");
    let install = command_env("cargo install --path dashboard/runtime --locked", target);
    let names: Vec<&str> = install.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "CARGO_TARGET_DIR",
            "CARGO_PROFILE_RELEASE_LTO",
            "CARGO_PROFILE_RELEASE_CODEGEN_UNITS"
        ]
    );
    assert!(
        command_env(
            "cargo build --release --target wasm32-unknown-unknown",
            target
        )
        .is_empty()
    );
}

#[test]
fn a_page_with_a_kept_crate_writes_its_files_and_builds_with_its_lock() {
    let docs = TempDir::new().expect("a folder");
    let page = docs.path().join("getting-started.md");
    fs::write(&page, "").unwrap();
    let crate_dir = docs.path().join("getting-started");
    fs::create_dir_all(crate_dir.join("src")).unwrap();
    fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"my-widget\"\n",
    )
    .unwrap();
    fs::write(crate_dir.join("Cargo.lock"), "version = 4\n").unwrap();
    fs::write(crate_dir.join("src/lib.rs"), "").unwrap();
    let mut kept = Kept::of(&page).expect("the folder beside the page");

    let differs = kept
        .check_file("Cargo.toml", "[package]\nname = \"other\"\n")
        .unwrap_err();
    assert!(differs.contains("differs"), "{differs}");
    assert!(
        kept.check_file("Cargo.toml", "[package]\nname = \"my-widget\"\n")
            .is_ok()
    );
    assert!(
        kept.check_file("broken.json", "{").is_ok(),
        "a file of the page's own"
    );
    let missing = kept.check_end().unwrap_err();
    assert!(
        missing.contains("src/lib.rs"),
        "a kept file the page never wrote: {missing}"
    );
    assert!(kept.check_file("src/lib.rs", "").is_ok());

    let session = TempDir::new().expect("a folder");
    let written = session.path().join("Cargo.toml");
    fs::write(&written, "").unwrap();
    kept.seed_lock(&written);
    assert_eq!(
        fs::read_to_string(session.path().join("Cargo.lock")).unwrap(),
        "version = 4\n"
    );
    assert!(kept.check_end().is_ok());
    fs::write(
        session.path().join("Cargo.lock"),
        "version = 4\n# resolved again\n",
    )
    .unwrap();
    let changed = kept.check_end().unwrap_err();
    assert!(changed.contains("cargo changed"), "{changed}");
    assert!(Kept::of(&docs.path().join("other.md")).is_none());
}
