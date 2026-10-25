//! What a credential costs (`specs/005-plugin-credentials/research.md`, R18): a request built
//! and checked with and without a credential, the header added to the hop and the echo check
//! over a response of 1 MiB, each the median of `--iterations` runs (default 100), and their
//! difference against SC-006's 5 ms; then the first read of an entry from the system's keychain,
//! which opens it, and the median of later reads. The entry is deleted at the end.
//!
//! The host's own code runs, without the network: the test seam cannot compile in a release
//! build. Run with `cargo run -p dashboard-bench --release --bin credential_cost`; on Linux, in
//! a session with an unlocked Secret Service.

use std::time::{Duration, Instant};

use runtime::measure::{KeychainEntry, RequestProbe};

/// A response of 1 MiB that does not hold the credential, as the check reads every byte of it.
const RESPONSE_BYTES: usize = 1024 * 1024;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let iterations = args
        .iter()
        .position(|arg| arg == "--iterations")
        .and_then(|at| args.get(at + 1))
        .map(|value| value.parse::<usize>().expect("a whole number"))
        .unwrap_or(100)
        .max(1);

    let response: Vec<u8> = (0..RESPONSE_BYTES)
        .map(|i| b"{\"title\":\"note\"},"[i % 17])
        .collect();
    let plain = RequestProbe::new(false);
    let with = RequestProbe::new(true);
    let request = median(iterations, || assert!(!plain.run(&response)));
    let with_credential = median(iterations, || assert!(!with.run(&response)));
    let added = with_credential.saturating_sub(request);
    println!("request_us: {}", request.as_micros());
    println!(
        "request_with_credential_us: {}",
        with_credential.as_micros()
    );
    println!(
        "credential_added_us: {} (SC-006: under 5000)",
        added.as_micros()
    );

    let entry = KeychainEntry::create("credential-cost")
        .unwrap_or_else(|error| panic!("writing to the system's keychain: {error}"));
    let reader = entry.reader();
    let started = Instant::now();
    reader.read().expect("the first read");
    println!("vault_first_read_us: {}", started.elapsed().as_micros());
    let later = median(iterations, || reader.read().expect("a read"));
    println!("vault_read_us: {}", later.as_micros());
}

fn median(iterations: usize, mut run: impl FnMut()) -> Duration {
    let mut times: Vec<Duration> = (0..iterations)
        .map(|_| {
            let started = Instant::now();
            run();
            started.elapsed()
        })
        .collect();
    times.sort();
    times[times.len() / 2]
}
