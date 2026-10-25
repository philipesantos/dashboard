#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
//! The secrets vault: credentials kept in the system's keychain through keyring-rs, and nowhere
//! else (`specs/005-plugin-credentials/research.md`, R5; constitution Principle IV). `store.rs`
//! is the only code that touches `keyring-core` or a store crate, as `clippy.toml` enforces.

mod store;
