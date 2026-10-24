//! Name resolution for connector requests (`specs/003-plugin-connectors/research.md`, R6, R15),
//! and the test seam that stands in for DNS and Mozilla's roots in the runtime's own tests.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

use super::address;

/// What `ureq` keeps of a resolution.
const MAX_ADDRESSES: usize = 16;

/// Which addresses an agent may connect to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// Public addresses only: the HTTPS agent.
    Public,
    /// This machine only: the plain HTTP agent of local connectors in development (FR-018).
    Loopback,
}

/// Resolves a destination's host and keeps only the addresses its agent may reach; the
/// connection is made to those addresses alone, so a name cannot change between the check and
/// the connection. Run at every connection, so a name that starts leading to a private address
/// after approval is refused at the call (research R6).
#[derive(Debug)]
pub(crate) struct PublicOnly {
    reach: Reach,
    /// Set when every address was refused, so the call fails as `non-public-address`.
    refused: Arc<Mutex<Option<String>>>,
    #[cfg(feature = "test-network")]
    seam: Option<Arc<TestNetwork>>,
    #[cfg(feature = "test-network")]
    lookups: Mutex<std::collections::HashMap<(String, u16), usize>>,
}

impl PublicOnly {
    pub(crate) fn new(
        reach: Reach,
        refused: Arc<Mutex<Option<String>>>,
        #[cfg(feature = "test-network")] seam: Option<Arc<TestNetwork>>,
    ) -> PublicOnly {
        PublicOnly {
            reach,
            refused,
            #[cfg(feature = "test-network")]
            seam,
            #[cfg(feature = "test-network")]
            lookups: Mutex::default(),
        }
    }

    fn allowed(&self, address: SocketAddr) -> bool {
        #[cfg(feature = "test-network")]
        if let Some(seam) = &self.seam
            && seam.public_for_tests.contains(&address)
        {
            return true;
        }
        match self.reach {
            Reach::Public => address::is_public(address.ip()),
            Reach::Loopback => address::is_loopback(address.ip()),
        }
    }

    /// The addresses the seam's route gives for this lookup, when the seam is in use.
    #[cfg(feature = "test-network")]
    fn seam_lookup(&self, host: &str, port: u16) -> Option<Result<Vec<SocketAddr>, ureq::Error>> {
        let seam = self.seam.as_ref()?;
        let Some(route) = seam
            .routes
            .iter()
            .find(|route| route.host == host && route.port == port)
        else {
            return Some(Err(ureq::Error::HostNotFound));
        };
        let mut lookups = self.lookups.lock().unwrap_or_else(PoisonError::into_inner);
        let count = lookups.entry((host.to_owned(), port)).or_insert(0);
        let answer = route
            .answers
            .get(*count)
            .or_else(|| route.answers.last())
            .cloned()
            .unwrap_or_default();
        *count += 1;
        Some(Ok(answer))
    }
}

impl Resolver for PublicOnly {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let host = uri.host().unwrap_or_default().to_owned();
        #[cfg(feature = "test-network")]
        let seam = {
            let port = uri
                .port_u16()
                .unwrap_or(if uri.scheme_str() == Some("https") {
                    443
                } else {
                    80
                });
            self.seam_lookup(&host, port)
        };
        #[cfg(not(feature = "test-network"))]
        let seam: Option<Result<Vec<SocketAddr>, ureq::Error>> = None;
        let candidates: Vec<SocketAddr> = match seam {
            Some(answer) => answer?,
            None => DefaultResolver::default()
                .resolve(uri, config, timeout)?
                .iter()
                .copied()
                .collect(),
        };
        // The system resolver gives the addresses with the destination's port; a seam route
        // gives the test server's
        let mut kept = self.empty();
        for address in candidates
            .into_iter()
            .filter(|address| self.allowed(*address))
            .take(MAX_ADDRESSES)
        {
            kept.push(address);
        }
        if kept.is_empty() {
            let what = match self.reach {
                Reach::Public => "public",
                Reach::Loopback => "on this machine",
            };
            *self.refused.lock().unwrap_or_else(PoisonError::into_inner) = Some(format!(
                "{host} leads to no address that is {what}; a connector never reaches the user's machine or local network"
            ));
            return Err(ureq::Error::HostNotFound);
        }
        Ok(kept)
    }
}

/// What the runtime's own tests give instead of DNS and Mozilla's roots
/// (`specs/003-plugin-connectors/research.md`, R15). Only the HTTPS agent uses it: the
/// loopback agent of local connectors keeps its production resolver, so loopback cases run as
/// they would in the CLI. The library refuses to compile with this feature outside debug
/// builds, and nothing in the CLI reaches it.
#[cfg(feature = "test-network")]
#[derive(Debug, Clone, Default)]
pub struct TestNetwork {
    /// Stands in for DNS: where a destination's host and port lead.
    pub routes: Vec<Route>,
    /// Addresses that count as public in the test, such as a test server on `127.0.0.1`; every
    /// other address is judged as in production.
    pub public_for_tests: Vec<SocketAddr>,
    /// The only root certificate trusted, as DER, given to the same verifier production uses.
    pub root: Vec<u8>,
    /// Catalog connectors added for the test, as JSON, checked as catalog connectors.
    pub catalog: Vec<String>,
}

/// Where one destination's host and port lead in a test.
#[cfg(feature = "test-network")]
#[derive(Debug, Clone)]
pub struct Route {
    /// The host in its normal form, as the connector's origin has it.
    pub host: String,
    pub port: u16,
    /// Each lookup returns the next answer, and the last one repeats, so a name can change
    /// between calls (the `rebinding` case).
    pub answers: Vec<Vec<SocketAddr>>,
}
