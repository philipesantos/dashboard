//! The official connector catalog, built into the host (`specs/003-plugin-connectors/research.md`,
//! R2, R16).
//!
//! A connector is a catalog connector only if it is in the list below, which embeds each file of
//! `connectors/catalog/` when the host is compiled: no file next to the binary can add one. Each
//! catalog connector lists in `reserves` the domains of its service, and every host under a
//! reserved domain belongs to the catalog, so no other connector can claim it (FR-005).

use std::sync::{Arc, OnceLock};

use crate::connector::{self, Connector, Source};
#[cfg(any(test, feature = "test-network"))]
use crate::error::RuntimeError;

/// Every catalog connector: its id, which is `catalog/<file name>`, and its file.
const BUILT_IN: &[(&str, &str)] = &[(
    "catalog/github",
    include_str!("../../connectors/catalog/github.json"),
)];

/// The catalog a runtime uses: the built-in one, plus, in the runtime's own tests, connectors
/// added through the test seam.
#[derive(Debug, Clone)]
pub(crate) struct Catalog {
    connectors: Vec<Arc<Connector>>,
}

impl Catalog {
    /// The catalog the host was built with. Its files are checked when the host is tested, so
    /// one that does not parse is a bug of the build, not of the user's machine.
    pub(crate) fn built_in() -> Catalog {
        static BUILT: OnceLock<Catalog> = OnceLock::new();
        BUILT
            .get_or_init(|| {
                let connectors = BUILT_IN
                    .iter()
                    .map(|(id, text)| {
                        let connector = Connector::parse(text.as_bytes(), Source::Catalog)
                            .unwrap_or_else(|error| panic!("{id}: {error}"));
                        assert_eq!(connector.id(), *id, "a catalog file's id is its name");
                        Arc::new(connector)
                    })
                    .collect();
                Catalog { connectors }
            })
            .clone()
    }

    /// The built-in catalog with `extra` connectors, given as JSON, each checked as a catalog
    /// connector: the test seam's way to exercise catalog rules with connectors written in a test.
    #[cfg(any(test, feature = "test-network"))]
    pub(crate) fn with_extra(extra: &[String]) -> Result<Catalog, RuntimeError> {
        let mut catalog = Catalog::built_in();
        for text in extra {
            let connector = Connector::parse(text.as_bytes(), Source::Catalog)?;
            catalog
                .connectors
                .retain(|existing| existing.id() != connector.id());
            catalog.connectors.push(Arc::new(connector));
        }
        Ok(catalog)
    }

    /// The catalog connector `id`.
    pub(crate) fn get(&self, id: &str) -> Option<&Arc<Connector>> {
        self.connectors
            .iter()
            .find(|connector| connector.id() == id)
    }

    /// The catalog connector and the domain that reserve `host`, whatever its port; compared in
    /// lowercase.
    pub(crate) fn reserving(&self, host: &str) -> Option<(&str, &str)> {
        let host = host.to_ascii_lowercase();
        self.connectors.iter().find_map(|connector| {
            connector
                .reserves()
                .iter()
                .find(|domain| connector::is_under(&host, domain))
                .map(|domain| (connector.id(), domain.as_str()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_connector_parses_with_its_file_name_as_id() {
        let catalog = Catalog::built_in();
        assert_eq!(catalog.connectors.len(), BUILT_IN.len());
        for (id, _) in BUILT_IN {
            assert!(catalog.get(id).is_some(), "{id}");
        }
        assert!(catalog.get("catalog/missing").is_none());
    }

    /// The built-in half of the white-box case `catalog-reserves` (FR-005): parsing already
    /// refuses a catalog connector whose reserves miss a host, and this checks it again over the
    /// list the host ships, so the rule cannot erode silently.
    #[test]
    fn the_reserves_of_every_built_in_connector_cover_each_host_it_declares() {
        for connector in &Catalog::built_in().connectors {
            assert!(!connector.reserves().is_empty(), "{}", connector.id());
            for origin in connector.destinations() {
                assert!(
                    connector
                        .reserves()
                        .iter()
                        .any(|domain| connector::is_under(origin.host(), domain)),
                    "{}: {} is not under its reserves",
                    connector.id(),
                    origin.host()
                );
            }
        }
    }

    /// Every sign-in address and page of a catalog credential is under the domains its connector
    /// reserves (`specs/005-plugin-credentials/research.md`, R19). No connector the host ships
    /// declares a credential yet, so this runs over the built-in list and over one added through
    /// the seam.
    #[test]
    fn the_sign_in_addresses_of_every_catalog_credential_are_under_its_reserves() {
        let with_account = r#"{
            "connector_version": 1, "id": "catalog/echo", "version": "1.0.0", "author": "Tests",
            "description": "An echo service", "reserves": ["echo.test"],
            "destinations": { "api": "https://api.echo.test" },
            "credential": { "kind": "account", "destinations": ["api"], "header": "Authorization",
                "scheme": "Bearer", "account": { "flow": "authorization-code", "client": "echo",
                "authorize": "https://login.echo.test/authorize", "token": "https://api.echo.test/token",
                "access": [{ "name": "echo", "level": "read" }],
                "access_page": "https://echo.test/apps" } },
            "operations": { "get": { "kind": "read", "method": "GET", "destination": "api",
                "path": "/", "response": { "format": "json" } } }
        }"#;
        let catalog = Catalog::with_extra(&[with_account.to_owned()]).unwrap();
        for connector in &catalog.connectors {
            let Some(account) = connector.credential().and_then(|c| c.account()) else {
                continue;
            };
            let mut addresses = vec![account.authorize(), account.token()];
            addresses.extend(account.revoke());
            addresses.extend(account.access_page());
            addresses.extend(account.revoke_page());
            for address in addresses {
                let host = address.host_str().unwrap_or_default();
                assert!(
                    connector
                        .reserves()
                        .iter()
                        .any(|domain| connector::is_under(host, domain)),
                    "{}: {address} is not under its reserves",
                    connector.id()
                );
            }
        }
        let outside = with_account.replace("https://echo.test/apps", "https://echo.example/apps");
        assert!(Catalog::with_extra(&[outside]).is_err());
    }

    #[test]
    fn github_reserves_every_host_under_github_com_and_nothing_else() {
        let catalog = Catalog::built_in();
        for host in [
            "api.github.com",
            "API.GitHub.com",
            "uploads.github.com",
            "github.com",
            "a.b.github.com",
        ] {
            assert_eq!(
                catalog.reserving(host),
                Some(("catalog/github", "github.com")),
                "{host}"
            );
        }
        for host in [
            "github.io",
            "notgithub.com",
            "github.com.evil.test",
            "githubcom",
            "com",
        ] {
            assert!(catalog.reserving(host).is_none(), "{host}");
        }
    }

    #[test]
    fn extra_connectors_are_checked_as_catalog_connectors() {
        let extra = r#"{
            "connector_version": 1, "id": "catalog/echo", "version": "1.0.0", "author": "Tests",
            "description": "An echo service", "reserves": ["echo.test"],
            "destinations": { "api": "https://api.echo.test" },
            "operations": { "get": { "kind": "read", "method": "GET", "destination": "api",
                "path": "/", "response": { "format": "json" } } }
        }"#;
        let catalog = Catalog::with_extra(&[extra.to_owned()]).unwrap();
        assert!(catalog.get("catalog/echo").is_some());
        assert!(catalog.get("catalog/github").is_some());
        assert!(catalog.reserving("api.echo.test").is_some());

        // The seam half of `catalog-reserves`: a destination outside its reserves is refused
        let outside = extra.replace(r#"["echo.test"]"#, r#"["other.test"]"#);
        let error = Catalog::with_extra(&[outside]).unwrap_err();
        assert!(error.detail().contains("api.echo.test"), "{error}");
    }
}
