//! The browser of the test-only CLI (`specs/006-contract-v1-docs/research.md`, R9): instead of
//! opening the system's browser at a sign-in address, it follows the test sign-in service's
//! page, which signs in, accepts and redirects to the loopback callback at once. It goes through
//! the seam, as a connector's requests do, and exists only in builds with it.

use std::io;

use crate::Limits;

use super::{Network, TestNetwork};

/// Opens `address` as a person who signs in and accepts at once would: a `GET` of the sign-in
/// address through the seam's routes and test root, then a `GET` of the loopback callback its
/// answer redirects to.
pub fn follow(test: &TestNetwork, address: &str) -> io::Result<()> {
    let session = Network::with_seam(test.clone()).session(&Limits::default());
    let page = session
        .https
        .get(address)
        .call()
        .map_err(io::Error::other)?;
    let callback = page
        .headers()
        .get("Location")
        .and_then(|location| location.to_str().ok())
        .ok_or_else(|| {
            io::Error::other(format!(
                "the sign-in page answered {} without a redirect",
                page.status()
            ))
        })?
        .to_owned();
    session
        .loopback
        .get(&callback)
        .call()
        .map_err(io::Error::other)?;
    Ok(())
}
