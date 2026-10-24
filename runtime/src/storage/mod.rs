#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
//! Plugin storage: the only code that opens a plugin's store and the only user of `rusqlite`
//! (`specs/004-plugin-storage/research.md`, R1, R15). `clippy.toml` forbids `rusqlite`
//! everywhere else, so reviewing this module reviews every SQL statement the host runs and every
//! hardening layer of the connection.

mod admin;
// Until the storage calls use it (tasks T021 and T047)
#[allow(dead_code)]
mod connection;
