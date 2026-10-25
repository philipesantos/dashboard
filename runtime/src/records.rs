//! The file discipline of the host's records (the grant record of feature 002, the accounts
//! record of feature 005): a lock file beside each record, shared to read and exclusive to
//! change, and writes that replace the record whole with an atomic rename.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

/// Takes the record's lock file, `<record>.lock`: shared to read, exclusive to change. The lock
/// is released when the returned file is dropped. `Err` names the lock file.
pub(crate) fn lock(path: &Path, exclusive: bool) -> Result<File, (PathBuf, io::Error)> {
    let mut lock_path = OsString::from(path.as_os_str());
    lock_path.push(".lock");
    let lock_path = PathBuf::from(lock_path);
    if exclusive && let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|error| (path.to_owned(), error))?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| (lock_path.clone(), error))?;
    let locked = if exclusive {
        file.lock()
    } else {
        file.lock_shared()
    };
    locked.map_err(|error| (lock_path, error))?;
    Ok(file)
}

/// Writes `bytes` beside `path`, syncs them, then renames them over `path`, so a crash leaves
/// either the old record or the new one, never part of each.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new(""));
    let mut temporary = NamedTempFile::new_in(dir)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}
