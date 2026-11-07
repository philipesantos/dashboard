//! `cargo xtask clean-space`: frees the disk that builds and tests leave behind without forcing a
//! full rebuild. Removes the incremental compilation caches, the test and example executables that
//! a newer build of the same name replaced (or that no build touched for a week), and
//! `plugins/target/`, which only a build run inside `plugins/` makes: `cargo xtask plugins`
//! builds in `target/plugins-build/`. The next build recompiles the workspace's own crates and
//! links the executables again; the dependencies stay built.

use std::collections::HashMap;
use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::Result;

/// An executable no build touched for this long belongs to a test or example that no longer
/// exists, or one that links again in seconds.
const STALE: Duration = Duration::from_hours(7 * 24);

pub fn run(root: &Path) -> Result<()> {
    let target = root.join("target");
    let mut freed = 0;
    for profile in ["debug", "release"] {
        let dir = target.join(profile);
        if !dir.is_dir() {
            continue;
        }
        // Cargo holds these locks while it builds in the directory: cleaning under a build would
        // break it, and holding them keeps a new one from starting until we are done.
        let Some(_locks) = lock_out_builds(&dir)? else {
            println!("{}: a build is running there; left as it is", dir.display());
            continue;
        };
        let incremental = dir.join("incremental");
        let size = dir_size(&incremental);
        if size > 0 {
            fs::remove_dir_all(&incremental)?;
            println!("{}: {}", incremental.display(), human(size));
            freed += size;
        }
        for executables in ["deps", "examples"] {
            let executables = dir.join(executables);
            let size = old_executables(&executables)?;
            if size > 0 {
                println!(
                    "{}: {} of old executables",
                    executables.display(),
                    human(size)
                );
                freed += size;
            }
        }
    }
    let plugins = root.join("plugins").join("target");
    let size = dir_size(&plugins);
    if size > 0 {
        fs::remove_dir_all(&plugins)?;
        println!("{}: {}", plugins.display(), human(size));
        freed += size;
    }
    println!("freed: {}", human(freed));
    Ok(())
}

/// Takes every lock Cargo keeps in a profile's directory, or `None` when a build holds one.
fn lock_out_builds(dir: &Path) -> Result<Option<Vec<File>>> {
    let mut locks = Vec::new();
    for name in [".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock"] {
        let path = dir.join(name);
        if !path.is_file() {
            continue;
        }
        let lock = File::options().read(true).write(true).open(&path)?;
        match lock.try_lock() {
            Ok(()) => locks.push(lock),
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
    }
    Ok(Some(locks))
}

/// Removes the `name-<hash>` executables in `dir` that a newer build of the same name replaced
/// or that no build touched for a week; returns the bytes freed.
fn old_executables(dir: &Path) -> Result<u64> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(0);
    };
    let mut by_name: HashMap<String, Vec<(PathBuf, SystemTime, u64)>> = HashMap::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = executable_name(&path) else {
            continue;
        };
        let meta = entry.metadata()?;
        if !meta.is_file() || !is_executable(&path, &meta) {
            continue;
        }
        by_name
            .entry(name)
            .or_default()
            .push((path, meta.modified()?, meta.len()));
    }
    let now = SystemTime::now();
    let mut freed = 0;
    for mut builds in by_name.into_values() {
        builds.sort_by_key(|(_, modified, _)| *modified);
        let newest = builds.len() - 1;
        for (i, (path, modified, len)) in builds.into_iter().enumerate() {
            let stale = now.duration_since(modified).unwrap_or_default() > STALE;
            if i < newest || stale {
                fs::remove_file(&path)?;
                freed += len;
            }
        }
    }
    Ok(freed)
}

/// `it-0123456789abcdef` (or `it-0123456789abcdef.exe`) → `it`. Libraries, dependency files and
/// debug info have another extension, so they are never taken for an executable.
fn executable_name(path: &Path) -> Option<String> {
    let file = path.file_name()?.to_str()?;
    let file = file.strip_suffix(".exe").unwrap_or(file);
    if file.contains('.') {
        return None;
    }
    let (name, hash) = file.rsplit_once('-')?;
    (hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| name.to_owned())
}

#[cfg(unix)]
fn is_executable(_: &Path, meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(path: &Path, _: &fs::Metadata) -> bool {
    path.extension().is_some_and(|extension| extension == "exe")
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => dir_size(&entry.path()),
            Ok(meta) => meta.len(),
            Err(_) => 0,
        })
        .sum()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a size in GB for a person to read"
)]
fn human(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

#[cfg(test)]
mod tests {
    use super::executable_name;
    use std::path::Path;

    #[test]
    fn only_hashed_executables_have_a_name() {
        assert_eq!(
            executable_name(Path::new("deps/it-0123456789abcdef")).as_deref(),
            Some("it")
        );
        assert_eq!(
            executable_name(Path::new("deps/it-0123456789abcdef.exe")).as_deref(),
            Some("it")
        );
        assert_eq!(
            executable_name(Path::new("examples/hello_world-0123456789abcdef")).as_deref(),
            Some("hello_world")
        );
        assert_eq!(
            executable_name(Path::new("deps/libruntime-0123456789abcdef.rlib")),
            None
        );
        assert_eq!(
            executable_name(Path::new("deps/it-0123456789abcdef.d")),
            None
        );
        assert_eq!(
            executable_name(Path::new("deps/it-0123456789abcdef.pdb")),
            None
        );
        assert_eq!(executable_name(Path::new("deps/it-xyz")), None);
    }
}
