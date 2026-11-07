//! `runtime-test-network`: the development CLI on the test seam, for the docs runner alone
//! (`specs/006-contract-v1-docs/contracts/cli.md`, research R9). It builds only with the
//! `test-network` feature, which refuses release builds, and is never installed. Its requests go
//! to the test services the file named by `DASHBOARD_TEST_NETWORK` routes them to, its sign-in
//! follows the test sign-in page instead of opening a browser, its vault is the development one
//! in the system's keychain, as each command is a process of its own, and its data directory and
//! grant record are the ones the file names, never the default development ones.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::OnceLock;

use serde_json::Value;

/// What the file names: the seam and the records of the session.
struct Setup {
    network: runtime::TestNetwork,
    data_dir: PathBuf,
    grants: PathBuf,
}

static SETUP: OnceLock<Setup> = OnceLock::new();

fn setup() -> &'static Setup {
    SETUP.get().expect("main reads the setup first")
}

/// Reads the file `DASHBOARD_TEST_NETWORK` names: `routes` (each `host`, `port` and
/// `addresses`), `public_for_tests`, `root` as the DER's bytes, `catalog`, `data_dir` and
/// `grants`.
fn read() -> Result<Setup, String> {
    let path = std::env::var_os("DASHBOARD_TEST_NETWORK")
        .ok_or("runtime-test-network needs DASHBOARD_TEST_NETWORK, the test network's file")?;
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", PathBuf::from(&path).display()))?;
    let file: Value =
        serde_json::from_str(&text).map_err(|error| format!("the test network's file: {error}"))?;
    let address = |value: &Value| -> Result<SocketAddr, String> {
        value
            .as_str()
            .and_then(|text| text.parse().ok())
            .ok_or_else(|| format!("{value} is not an address"))
    };
    let list = |name: &str| -> Result<&Vec<Value>, String> {
        file[name]
            .as_array()
            .ok_or_else(|| format!("the test network's file has no {name}"))
    };
    let path_of = |name: &str| -> Result<PathBuf, String> {
        file[name]
            .as_str()
            .map(PathBuf::from)
            .ok_or_else(|| format!("the test network's file has no {name}"))
    };
    let mut routes = Vec::new();
    for route in list("routes")? {
        let addresses = route["addresses"]
            .as_array()
            .ok_or("a route has no addresses")?
            .iter()
            .map(address)
            .collect::<Result<Vec<_>, _>>()?;
        routes.push(runtime::Route {
            host: route["host"]
                .as_str()
                .ok_or("a route has no host")?
                .to_owned(),
            port: route["port"]
                .as_u64()
                .and_then(|port| u16::try_from(port).ok())
                .ok_or("a route has no port")?,
            answers: vec![addresses],
        });
    }
    let root = list("root")?
        .iter()
        .map(|byte| byte.as_u64().and_then(|byte| u8::try_from(byte).ok()))
        .collect::<Option<Vec<u8>>>()
        .ok_or("root is not a list of bytes")?;
    let catalog = list("catalog")?
        .iter()
        .map(|connector| connector.as_str().map(str::to_owned))
        .collect::<Option<Vec<String>>>()
        .ok_or("catalog is not a list of connectors as text")?;
    Ok(Setup {
        network: runtime::TestNetwork {
            routes,
            public_for_tests: list("public_for_tests")?
                .iter()
                .map(address)
                .collect::<Result<_, _>>()?,
            root,
            catalog,
        },
        data_dir: path_of("data_dir")?,
        grants: path_of("grants")?,
    })
}

fn main() -> ExitCode {
    match read() {
        Ok(read) => {
            let _ = SETUP.set(read);
        }
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    }
    runtime::cli::main_with(runtime::cli::Host {
        start: |limits, options| {
            runtime::Runtime::with_test_network_on_keychain(
                limits,
                options,
                setup().network.clone(),
            )
        },
        open: |address| runtime::follow_test_sign_in(&setup().network, address),
        data_dir: || Some(setup().data_dir.clone()),
        grants_path: || Some(setup().grants.clone()),
    })
}
