//! What `bench/src/bin/credential_cost.rs` measures, in release builds, where the test seam
//! cannot compile (`specs/005-plugin-credentials/research.md`, R18): the host's own code for
//! adding a credential to a request and looking for it in a response, without the network, and
//! reads of the development vault in the system's keychain. The bench times them; nothing here
//! times anything, and nothing here is part of the host's interface.

use std::time::Duration;

use crate::connector::{Connector, Method, Source};
use crate::network::{Request, RequestCredential, http_request};
use crate::vault::{self, Backend, Secret, Stored, Vault};

/// The connector of the measurements: an account sent as `Authorization: Bearer` to
/// `https://bench.test`.
const CONNECTOR: &str = r#"{
  "connector_version": 1, "id": "local/bench", "version": "1.0.0", "author": "Dashboard contributors",
  "description": "What a credential costs a request",
  "destinations": { "api": "https://bench.test" },
  "credential": {
    "kind": "account", "required": false, "destinations": ["api"],
    "header": "Authorization", "scheme": "Bearer",
    "account": {
      "flow": "authorization-code", "client": "bench",
      "authorize": "https://bench.test/authorize", "token": "https://bench.test/token",
      "access": [{ "name": "notes", "level": "read" }]
    }
  },
  "operations": {
    "list": { "kind": "read", "method": "GET", "destination": "api", "path": "/notes",
              "response": { "format": "json" } }
  }
}"#;

/// A token of the length services issue, in the bearer-token set.
const TOKEN: &str = "at_bench.0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJ";

/// One request to `https://bench.test/notes`, with or without the credential.
pub struct RequestProbe {
    request: Request,
}

impl RequestProbe {
    pub fn new(with_credential: bool) -> RequestProbe {
        let connector =
            Connector::parse(CONNECTOR.as_bytes(), Source::Local).expect("the bench connector");
        let credential = with_credential.then(|| {
            RequestCredential::new(
                connector.credential().expect("it declares a credential"),
                &Secret::new(TOKEN.to_owned()),
            )
            .expect("the token is a bearer token")
        });
        RequestProbe {
            request: Request {
                method: Method::Get,
                url: url::Url::parse("https://bench.test/notes").expect("an address"),
                headers: vec![("Accept".to_owned(), "application/json".to_owned())],
                body: None,
                credential,
            },
        }
    }

    /// Builds the request of one hop, as a call sends it, then looks for the credential in
    /// `response`, as a call does with what comes back. True when it was found.
    pub fn run(&self, response: &[u8]) -> bool {
        let built = http_request(&self.request).expect("the request builds");
        std::hint::black_box(&built);
        self.request
            .credential
            .as_ref()
            .is_some_and(|credential| credential.is_in(response))
    }
}

/// An entry of the development vault in the system's keychain, under a user name of its own,
/// deleted when dropped.
pub struct KeychainEntry {
    vault: Vault,
    user: String,
}

/// What each keychain operation waits, as the development tool does.
const WAIT: Duration = Duration::from_secs(120);

impl KeychainEntry {
    /// Writes a key to a new entry, named after `name` and this process.
    pub fn create(name: &str) -> Result<KeychainEntry, String> {
        let limits = crate::Limits::default();
        let vault = Vault::new(Backend::System, limits.max_secret_bytes);
        let user = vault::entry_user(
            &format!("bench-{}", std::process::id()),
            "local/bench",
            name,
        );
        let stored = Stored::Key {
            key: Secret::new(TOKEN.to_owned()),
        };
        vault
            .write(&user, &stored)
            .within(WAIT)
            .map_err(|error| error.to_string())?;
        Ok(KeychainEntry { vault, user })
    }

    /// A reader of the entry with a vault of its own, whose first read opens the keychain, as
    /// the first read of a new process does.
    pub fn reader(&self) -> KeychainReader {
        KeychainReader {
            vault: Vault::new(Backend::System, crate::Limits::default().max_secret_bytes),
            user: self.user.clone(),
        }
    }
}

impl Drop for KeychainEntry {
    fn drop(&mut self) {
        let _ = self.vault.delete(&self.user).within(WAIT);
    }
}

pub struct KeychainReader {
    vault: Vault,
    user: String,
}

impl KeychainReader {
    pub fn read(&self) -> Result<(), String> {
        self.vault
            .read(&self.user)
            .within(WAIT)
            .map(drop)
            .map_err(|error| error.to_string())
    }
}
