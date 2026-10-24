#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
//! Test services for connector calls (`specs/003-plugin-connectors/research.md`, R15): HTTPS and
//! plain HTTP servers on `127.0.0.1`, each on a port the system chose, scripted per path and
//! recording every request they receive, and the certificates they present.
//!
//! These are the only test code that opens sockets, so this module allows what `clippy.toml`
//! forbids everywhere outside `runtime/src/network/`. A server that cannot start panics: a
//! hostile case must fail, never skip, or a broken rule would pass (Principle VIII).

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, CrlDistributionPoint, CustomExtension,
    IsCa, KeyPair, KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// A request a test server received, as it arrived.
#[derive(Debug, Clone)]
pub struct Received {
    pub method: String,
    /// The request target as sent: path and query.
    pub target: String,
    pub path: String,
    pub query: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub at: Instant,
}

impl Received {
    /// The value of the header `name`, compared without case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// What a server answers on one path.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Waited before answering, after reading the request.
    pub delay: Duration,
}

impl Reply {
    /// A `200` with a JSON body.
    pub fn json(body: &str) -> Reply {
        Reply::status(200)
            .header("Content-Type", "application/json")
            .body(body.as_bytes().to_vec())
    }

    /// A `200` with a plain-text body.
    pub fn text(body: &str) -> Reply {
        Reply::status(200)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(body.as_bytes().to_vec())
    }

    /// A JSON body of exactly `size` bytes.
    pub fn json_of_size(size: usize) -> Reply {
        let mut body = String::from("{\"pad\":\"\"}");
        body.insert_str(body.len() - 2, &"x".repeat(size.saturating_sub(body.len())));
        Reply::json(&body)
    }

    pub fn status(status: u16) -> Reply {
        Reply {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            delay: Duration::ZERO,
        }
    }

    /// A redirect to `location`, which may be relative.
    pub fn redirect(status: u16, location: &str) -> Reply {
        Reply::status(status).header("Location", location)
    }

    pub fn header(mut self, name: &str, value: &str) -> Reply {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn body(mut self, body: Vec<u8>) -> Reply {
        self.body = body;
        self
    }

    pub fn delay(mut self, delay: Duration) -> Reply {
        self.delay = delay;
        self
    }
}

/// A certificate and key a test HTTPS server presents.
pub struct ServerCert {
    chain: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

impl Clone for ServerCert {
    fn clone(&self) -> Self {
        ServerCert {
            chain: self.chain.clone(),
            key: self.key.clone_key(),
        }
    }
}

/// A test certificate authority, whose root the runtime trusts through the test seam, and a
/// second one it never trusts.
pub struct TestPki {
    ca: CertifiedIssuer<'static, KeyPair>,
    other: CertifiedIssuer<'static, KeyPair>,
}

impl Default for TestPki {
    fn default() -> Self {
        TestPki::new()
    }
}

impl TestPki {
    pub fn new() -> TestPki {
        TestPki {
            ca: authority("Dashboard test CA"),
            other: authority("Another CA"),
        }
    }

    /// The root certificate the runtime trusts in tests, as DER.
    pub fn root(&self) -> Vec<u8> {
        self.ca.der().to_vec()
    }

    /// A valid certificate for `names`, which the test seam's root trusts.
    pub fn leaf(&self, names: &[&str]) -> ServerCert {
        leaf_signed_by(&self.ca, names, |_| {})
    }

    /// A certificate for `names` signed by a CA the runtime does not trust.
    pub fn untrusted(&self, names: &[&str]) -> ServerCert {
        leaf_signed_by(&self.other, names, |_| {})
    }

    /// A certificate for `names` that expired in 2021.
    pub fn expired(&self, names: &[&str]) -> ServerCert {
        leaf_signed_by(&self.ca, names, |params| {
            params.not_before = rcgen::date_time_ymd(2020, 1, 1);
            params.not_after = rcgen::date_time_ymd(2021, 1, 1);
        })
    }

    /// A valid certificate for `names` whose OCSP responder, issuer certificate and CRL are on
    /// `fetch_base` (such as `http://127.0.0.1:<port>`): a verifier that fetched them would
    /// reach that server.
    pub fn with_fetch_addresses(&self, names: &[&str], fetch_base: &str) -> ServerCert {
        let ocsp = format!("{fetch_base}/ocsp");
        let issuer = format!("{fetch_base}/issuer.der");
        let crl = format!("{fetch_base}/crl.der");
        leaf_signed_by(&self.ca, names, |params| {
            params
                .custom_extensions
                .push(authority_information_access(&ocsp, &issuer));
            params.crl_distribution_points = vec![CrlDistributionPoint { uris: vec![crl] }];
        })
    }
}

fn authority(name: &str) -> CertifiedIssuer<'static, KeyPair> {
    let mut params = CertificateParams::new(Vec::<String>::new()).expect("CA parameters");
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, name);
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let key = KeyPair::generate().expect("a CA key");
    CertifiedIssuer::self_signed(params, key).expect("a self-signed CA")
}

fn leaf_signed_by(
    issuer: &CertifiedIssuer<'static, KeyPair>,
    names: &[&str],
    edit: impl FnOnce(&mut CertificateParams),
) -> ServerCert {
    let names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    let mut params = CertificateParams::new(names).expect("leaf parameters");
    params.use_authority_key_identifier_extension = true;
    edit(&mut params);
    let key = KeyPair::generate().expect("a leaf key");
    let certificate = params.signed_by(&key, issuer).expect("a signed leaf");
    ServerCert {
        chain: vec![certificate.der().clone()],
        key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
    }
}

/// The Authority Information Access extension (RFC 5280, 4.2.2.1) with an OCSP responder and a
/// CA issuer address, in DER.
fn authority_information_access(ocsp: &str, issuer: &str) -> CustomExtension {
    /// id-ad-ocsp and id-ad-caIssuers, 1.3.6.1.5.5.7.48.1 and .2
    const OCSP: &[u8] = &[0x06, 0x08, 0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x01];
    const CA_ISSUERS: &[u8] = &[0x06, 0x08, 0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x02];
    let description = |method: &[u8], uri: &str| {
        let mut content = method.to_vec();
        content.extend(der(0x86, uri.as_bytes()));
        der(0x30, &content)
    };
    let mut descriptions = description(OCSP, ocsp);
    descriptions.extend(description(CA_ISSUERS, issuer));
    CustomExtension::from_oid_content(&[1, 3, 6, 1, 5, 5, 7, 1, 1], der(0x30, &descriptions))
}

/// A DER element: tag, length, content.
fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut element = vec![tag];
    let length = content.len();
    if length < 0x80 {
        element.push(length as u8);
    } else {
        let bytes: Vec<u8> = length
            .to_be_bytes()
            .into_iter()
            .skip_while(|byte| *byte == 0)
            .collect();
        element.push(0x80 | bytes.len() as u8);
        element.extend(bytes);
    }
    element.extend_from_slice(content);
    element
}

/// A test server on `127.0.0.1`: answers each path as scripted (404 otherwise) and records
/// every request. Each connection serves one request and closes.
pub struct Server {
    address: SocketAddr,
    routes: Arc<Mutex<HashMap<String, Reply>>>,
    received: Arc<Mutex<Vec<Received>>>,
    stopped: Arc<AtomicBool>,
}

impl Server {
    /// A plain HTTP server.
    pub fn http() -> Server {
        Server::start(None)
    }

    /// An HTTPS server presenting `cert`.
    pub fn https(cert: &ServerCert) -> Server {
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .with_no_client_auth()
        .with_single_cert(cert.chain.clone(), cert.key.clone_key())
        .expect("a server certificate");
        Server::start(Some(Arc::new(config)))
    }

    /// A server that records whatever reaches it, such as a request sent to a network proxy;
    /// it answers 502 to everything.
    pub fn recorder() -> Server {
        Server::http()
    }

    fn start(tls: Option<Arc<rustls::ServerConfig>>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|error| panic!("a test server cannot start: {error}"));
        listener
            .set_nonblocking(true)
            .expect("a non-blocking listener");
        let address = listener.local_addr().expect("the server's address");
        let server = Server {
            address,
            routes: Arc::default(),
            received: Arc::default(),
            stopped: Arc::default(),
        };
        let routes = Arc::clone(&server.routes);
        let received = Arc::clone(&server.received);
        let stopped = Arc::clone(&server.stopped);
        thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let routes = Arc::clone(&routes);
                        let received = Arc::clone(&received);
                        let tls = tls.clone();
                        thread::spawn(move || {
                            let _ = serve(stream, tls, &routes, &received);
                        });
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        server
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn port(&self) -> u16 {
        self.address.port()
    }

    /// `http://127.0.0.1:<port>`
    pub fn http_origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port())
    }

    /// Answers `path` (without its query) with `reply` from now on.
    pub fn route(&self, path: &str, reply: Reply) {
        self.routes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(path.to_owned(), reply);
    }

    /// Every request received so far, oldest first.
    pub fn received(&self) -> Vec<Received> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Stops accepting connections: the service is down from now on.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        // Let the accept loop see the flag and drop the listener.
        thread::sleep(Duration::from_millis(50));
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
    }
}

/// Reads one request, records it and answers it.
fn serve(
    stream: TcpStream,
    tls: Option<Arc<rustls::ServerConfig>>,
    routes: &Mutex<HashMap<String, Reply>>,
    received: &Mutex<Vec<Received>>,
) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    match tls {
        None => {
            let reader = stream.try_clone()?;
            exchange(reader, stream, routes, received)
        }
        Some(config) => {
            let connection = rustls::ServerConnection::new(config).map_err(io::Error::other)?;
            let tls = Arc::new(Mutex::new(rustls::StreamOwned::new(connection, stream)));
            exchange(Shared(Arc::clone(&tls)), Shared(tls), routes, received)
        }
    }
}

/// One TLS stream used for reading and writing in turn.
struct Shared<S>(Arc<Mutex<S>>);

impl<S: Read> Read for Shared<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .read(buf)
    }
}

impl<S: Write> Write for Shared<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .flush()
    }
}

fn exchange(
    reader: impl Read,
    mut writer: impl Write,
    routes: &Mutex<HashMap<String, Reply>>,
    received: &Mutex<Vec<Received>>,
) -> io::Result<()> {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(());
    }
    let mut parts = line.trim_end().splitn(3, ' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            break;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path.to_owned(), Some(query.to_owned())),
        None => (target.clone(), None),
    };
    received
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Received {
            method: method.clone(),
            target: target.clone(),
            path: path.clone(),
            query,
            headers,
            body,
            at: Instant::now(),
        });

    let reply = if method == "CONNECT" {
        Reply::status(502)
    } else {
        routes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&path)
            .cloned()
            .unwrap_or_else(|| Reply::status(404))
    };
    thread::sleep(reply.delay);
    let mut response = format!("HTTP/1.1 {} Test\r\n", reply.status);
    for (name, value) in &reply.headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    if method != "HEAD" {
        response.push_str(&format!("Content-Length: {}\r\n", reply.body.len()));
    }
    response.push_str("Connection: close\r\n\r\n");
    writer.write_all(response.as_bytes())?;
    if method != "HEAD" {
        writer.write_all(&reply.body)?;
    }
    writer.flush()?;
    Ok(())
}
