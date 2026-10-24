//! Name resolution for connector requests (`specs/003-plugin-connectors/research.md`, R6, R15),
//! and the test seam that stands in for DNS and Mozilla's roots in the runtime's own tests.

#[cfg(feature = "test-network")]
use std::net::SocketAddr;

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
