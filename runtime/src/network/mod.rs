#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
//! The network module: the only code that opens connections and the only user of `ureq`
//! (`specs/003-plugin-connectors/research.md`, R5–R10, R20). `clippy.toml` forbids `ureq` and
//! sockets everywhere else, so reviewing this module reviews every byte a connector sends.
//!
//! Each update gets a [`Session`] of two agents, dropped when the update ends: one for HTTPS,
//! which reaches only public addresses and trusts only Mozilla's roots, and one for the plain
//! HTTP of local connectors on this machine, which reaches only loopback addresses. Neither
//! uses a network proxy, keeps cookies, asks for compression or follows a redirect: the host
//! follows redirects itself, to the connector's own origins only.

mod address;
mod resolver;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;
use ureq::Agent;
use ureq::config::AutoHeaderValue;
use ureq::http;
#[cfg(feature = "test-network")]
use ureq::tls::Certificate;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::unversioned::transport::DefaultConnector;

use crate::connector::{ArgValue, Connector, Format, Method, Operation, Template};
use crate::limits::Limits;

#[cfg(feature = "test-network")]
pub use resolver::{Route, TestNetwork};

/// Said when a call fails as unavailable: behind a required proxy, this is why (clarification 5).
const NO_PROXY_NOTE: &str = "the host connects directly and does not use network proxies";

/// How the runtime reaches connector destinations: production DNS and Mozilla's roots, or, in
/// the runtime's own tests, the test seam.
#[derive(Debug, Clone, Default)]
pub(crate) struct Network {
    #[cfg(feature = "test-network")]
    seam: Option<Arc<TestNetwork>>,
}

impl Network {
    pub(crate) fn production() -> Network {
        Network::default()
    }

    #[cfg(feature = "test-network")]
    pub(crate) fn with_seam(seam: TestNetwork) -> Network {
        Network {
            seam: Some(Arc::new(seam)),
        }
    }

    /// The agents of one update; nothing in them outlives it.
    pub(crate) fn session(&self, limits: &Limits) -> Session {
        let refused = Arc::new(Mutex::new(None));
        #[cfg(feature = "test-network")]
        let (roots, seam) = match &self.seam {
            Some(seam) => (
                RootCerts::Specific(Arc::new(vec![Certificate::from_der(&seam.root).to_owned()])),
                Some(Arc::clone(seam)),
            ),
            None => (RootCerts::WebPki, None),
        };
        #[cfg(not(feature = "test-network"))]
        let roots = RootCerts::WebPki;

        let https = resolver::PublicOnly::new(
            resolver::Reach::Public,
            Arc::clone(&refused),
            #[cfg(feature = "test-network")]
            seam,
        );
        let loopback = resolver::PublicOnly::new(
            resolver::Reach::Loopback,
            Arc::clone(&refused),
            #[cfg(feature = "test-network")]
            None,
        );
        Session {
            https: agent(limits, true, roots.clone(), https),
            loopback: agent(limits, false, roots, loopback),
            refused,
            limits: limits.clone(),
        }
    }
}

/// An agent with every default that matters set explicitly (research R5).
fn agent(
    limits: &Limits,
    https_only: bool,
    roots: RootCerts,
    resolver: resolver::PublicOnly,
) -> Agent {
    let tls = TlsConfig::builder()
        .provider(TlsProvider::Rustls)
        .root_certs(roots)
        .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .build();
    let config = Agent::config_builder()
        .https_only(https_only)
        // The default reads HTTPS_PROXY and the like; behind a proxy the host could not tell
        // which address a name leads to (R9)
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(AutoHeaderValue::Provided(Arc::new(format!(
            "dashboard/{}",
            env!("CARGO_PKG_VERSION")
        ))))
        // No compression is compiled in; ask for none, so the size limits apply to what arrives
        .accept_encoding(AutoHeaderValue::None)
        .max_response_header_size(limits.max_response_header_bytes)
        .tls_config(tls)
        .build();
    Agent::with_parts(config, DefaultConnector::new(), resolver)
}

/// A request built from an operation and checked arguments, ready to send.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    pub(crate) method: Method,
    pub(crate) url: url::Url,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Option<Vec<u8>>,
}

impl Request {
    /// Path and query, as logged.
    pub(crate) fn target(&self) -> String {
        match self.url.query() {
            Some(query) => format!("{}?{query}", self.url.path()),
            None => self.url.path().to_owned(),
        }
    }

    pub(crate) fn host(&self) -> String {
        self.url.host_str().unwrap_or_default().to_owned()
    }
}

/// Why the host refused to build a request: always a size limit (`refused(over-limit)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refusal(pub(crate) String);

/// Builds the request of `operation` with `arguments` (checked by
/// [`Operation::check_arguments`]), encoding every value in its own place (research R3): a path
/// value percent-encoded except unreserved characters, a query value form-encoded, a body value
/// as a JSON value of its type. An absent optional parameter drops its query pair or body field.
pub(crate) fn build_request(
    connector: &Connector,
    operation: &Operation,
    arguments: &BTreeMap<String, ArgValue>,
    limits: &Limits,
) -> Result<Request, Refusal> {
    let origin = connector
        .destination(operation.destination())
        .expect("a checked operation's destination is declared");
    let mut path = String::new();
    for segment in operation.path() {
        path.push('/');
        match segment {
            Template::Literal(text) => path.push_str(text),
            Template::Parameter(name) => {
                let value = arguments
                    .get(name)
                    .expect("a path parameter is required, so checked arguments hold it");
                path.push_str(&encode_segment(&text_of(value)));
            }
        }
    }
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    let mut any_pair = false;
    for (name, value) in operation.query() {
        let value = match value {
            Template::Literal(text) => Some(text.clone()),
            Template::Parameter(parameter) => arguments.get(parameter).map(text_of),
        };
        if let Some(value) = value {
            query.append_pair(name, &value);
            any_pair = true;
        }
    }
    let query = query.finish();
    let target = if any_pair {
        format!("{path}?{query}")
    } else {
        path
    };
    if target.len() > limits.max_request_address_bytes {
        return Err(Refusal(format!(
            "the request address is {} bytes, over the limit of {}",
            target.len(),
            limits.max_request_address_bytes
        )));
    }
    let url = url::Url::parse(&format!("{}{target}", origin.serialization()))
        .map_err(|error| Refusal(format!("the request address is not valid: {error}")))?;

    let body = match operation.body() {
        None => None,
        Some(template) => {
            let body = fill_body(template, arguments)
                .unwrap_or(Value::Null)
                .to_string();
            if body.len() > limits.max_request_body_bytes {
                return Err(Refusal(format!(
                    "the request body is {} bytes, over the limit of {}",
                    body.len(),
                    limits.max_request_body_bytes
                )));
            }
            Some(body.into_bytes())
        }
    };
    let mut headers = operation.headers().to_vec();
    if body.is_some() {
        headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
    }
    Ok(Request {
        method: operation.method(),
        url,
        headers,
        body,
    })
}

fn text_of(value: &ArgValue) -> String {
    match value {
        ArgValue::Text(text) => text.clone(),
        ArgValue::Integer(number) => number.to_string(),
        ArgValue::Flag(flag) => flag.to_string(),
    }
}

/// RFC 3986 unreserved characters stay; every other byte of the UTF-8 is percent-encoded, `/`,
/// `%`, `.` sequences and all, so a value never leaves its segment.
fn encode_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// The body with each whole placeholder string replaced by its argument as a JSON value; a field
/// or item whose optional argument is absent is removed. `None` when `template` itself is such a
/// placeholder.
fn fill_body(template: &Value, arguments: &BTreeMap<String, ArgValue>) -> Option<Value> {
    match template {
        Value::String(text) => match crate::connector::placeholder_name(text) {
            Some(name) => arguments.get(name).map(|value| match value {
                ArgValue::Text(text) => Value::String(text.clone()),
                ArgValue::Integer(number) => Value::from(*number),
                ArgValue::Flag(flag) => Value::Bool(*flag),
            }),
            None => Some(template.clone()),
        },
        Value::Array(items) => Some(Value::Array(
            items
                .iter()
                .filter_map(|item| fill_body(item, arguments))
                .collect(),
        )),
        Value::Object(fields) => Some(Value::Object(
            fields
                .iter()
                .filter_map(|(name, item)| {
                    fill_body(item, arguments).map(|item| (name.clone(), item))
                })
                .collect(),
        )),
        other => Some(other.clone()),
    }
}

/// The rule behind a call that was attempted and produced no response for the widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureRule {
    OverLimit,
    NonPublicAddress,
    RedirectNotAllowed,
    Unavailable,
    InvalidResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
    pub(crate) rule: FailureRule,
    pub(crate) detail: String,
}

impl Failure {
    fn new(rule: FailureRule, detail: impl Into<String>) -> Failure {
        Failure {
            rule,
            detail: detail.into(),
        }
    }
}

/// What a service answered, as the widget receives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Response {
    pub(crate) status: u16,
    /// Each header the operation declares, in its order, `None` when absent.
    pub(crate) headers: Vec<(String, Option<String>)>,
    pub(crate) body: Vec<u8>,
}

/// One request that left the machine, or was about to: the first one of a call, or a redirect
/// the host followed. Each gets its own entry in the request log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hop {
    pub(crate) method: &'static str,
    pub(crate) host: String,
    pub(crate) target: String,
    /// The status the service answered, or why this request failed.
    pub(crate) outcome: Result<u16, Failure>,
    pub(crate) sent_bytes: u64,
    pub(crate) received_bytes: u64,
    pub(crate) duration: Duration,
}

/// Everything one call did on the network.
#[derive(Debug)]
pub(crate) struct Exchange {
    pub(crate) hops: Vec<Hop>,
    pub(crate) result: Result<Response, Failure>,
}

/// The agents of one update.
pub(crate) struct Session {
    https: Agent,
    loopback: Agent,
    /// Set by a resolver that found only addresses it may not reach, so the error can say so.
    refused: Arc<Mutex<Option<String>>>,
    limits: Limits,
}

impl Session {
    /// Sends `request`, following the redirects the connector allows, within the call's time
    /// limit and `deadline`, the end of the update. `update_bytes_left` is what remains of the
    /// update's response budget; the body received is taken from it.
    pub(crate) fn send(
        &self,
        connector: &Connector,
        operation: &Operation,
        mut request: Request,
        deadline: Instant,
        update_bytes_left: &mut usize,
    ) -> Exchange {
        let start = Instant::now();
        let call_deadline = (start + self.limits.call_time_limit).min(deadline);
        let mut hops = Vec::new();
        for redirect in 0..=self.limits.max_redirects {
            let hop_start = Instant::now();
            let Some(left) = call_deadline
                .checked_duration_since(hop_start)
                .filter(|left| !left.is_zero())
            else {
                return Exchange {
                    hops,
                    result: Err(self.out_of_time(deadline)),
                };
            };
            let sent_bytes = sent_bytes(&request);
            let mut hop = Hop {
                method: request.method.as_str(),
                host: request.host(),
                target: request.target(),
                outcome: Ok(0),
                sent_bytes,
                received_bytes: 0,
                duration: Duration::ZERO,
            };
            let answer = self.once(&request, left, deadline, update_bytes_left, operation);
            hop.duration = hop_start.elapsed();
            let answer = match answer {
                Ok(answer) => answer,
                Err(mut failure) => {
                    // A redirect that leads to a non-public address is one not followed
                    // (FR-019): the target is the service's choice, not the connector's
                    if redirect > 0 && failure.rule == FailureRule::NonPublicAddress {
                        failure = Failure::new(
                            FailureRule::RedirectNotAllowed,
                            format!("to {}: {}", hop.host, failure.detail),
                        );
                    }
                    hop.outcome = Err(failure.clone());
                    hops.push(hop);
                    return Exchange {
                        hops,
                        result: Err(failure),
                    };
                }
            };
            let status = answer.response.status;
            hop.outcome = Ok(status);
            hop.received_bytes = answer.response.body.len() as u64;
            hops.push(hop);

            let Some(location) = answer.location.filter(|_| is_redirect(status)) else {
                return Exchange {
                    hops,
                    result: check_format(
                        operation.response().format,
                        answer.content_type.as_deref(),
                        answer.response,
                    ),
                };
            };
            if redirect == self.limits.max_redirects {
                break;
            }
            match follow(connector, &request, status, &location) {
                Ok(next) => request = next,
                Err(failure) => {
                    return Exchange {
                        hops,
                        result: Err(failure),
                    };
                }
            }
        }
        Exchange {
            hops,
            result: Err(Failure::new(
                FailureRule::RedirectNotAllowed,
                format!(
                    "more than {} redirects; the host follows no more",
                    self.limits.max_redirects
                ),
            )),
        }
    }

    fn out_of_time(&self, deadline: Instant) -> Failure {
        if Instant::now() >= deadline {
            Failure::new(
                FailureRule::OverLimit,
                format!(
                    "the update reached its time limit of {:?}",
                    self.limits.update_time_limit
                ),
            )
        } else {
            Failure::new(
                FailureRule::OverLimit,
                format!(
                    "longer than the call's time limit of {:?}",
                    self.limits.call_time_limit
                ),
            )
        }
    }

    /// One request, no redirect followed.
    fn once(
        &self,
        request: &Request,
        timeout: Duration,
        deadline: Instant,
        update_bytes_left: &mut usize,
        operation: &Operation,
    ) -> Result<Answer, Failure> {
        let agent = if request.url.scheme() == "https" {
            &self.https
        } else {
            &self.loopback
        };
        *self.refused.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let mut builder = http::Request::builder()
            .method(request.method.as_str())
            .uri(request.url.as_str());
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let body = request.body.clone().unwrap_or_default();
        let http_request = builder
            .body(body)
            .map_err(|error| Failure::new(FailureRule::Unavailable, format!("{error}")))?;
        let http_request = agent
            .configure_request(http_request)
            .timeout_global(Some(timeout))
            .build();
        let mut response = agent
            .run(http_request)
            .map_err(|error| self.failure(error, request, deadline))?;

        let status = response.status().as_u16();
        let headers = operation
            .response()
            .headers
            .iter()
            .map(|name| {
                let values: Vec<String> = response
                    .headers()
                    .get_all(name.as_str())
                    .iter()
                    .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
                    .collect();
                (
                    name.clone(),
                    (!values.is_empty()).then(|| values.join(", ")),
                )
            })
            .collect();
        let location = response
            .headers()
            .get("location")
            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned());
        let content_type = response
            .headers()
            .get("content-type")
            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned());

        let body = if request.method == Method::Head {
            Vec::new()
        } else {
            let per_call = self.limits.max_response_body_bytes;
            let limit = per_call.min(*update_bytes_left);
            // One byte of slack: ureq refuses to read on once its limit is reached, even to see
            // the end, so a body of exactly the limit is read whole and accepted
            let read = response
                .body_mut()
                .with_config()
                .limit(limit as u64 + 1)
                .read_to_vec()
                .and_then(|body| {
                    if body.len() > limit {
                        Err(ureq::Error::BodyExceedsLimit(limit as u64))
                    } else {
                        Ok(body)
                    }
                });
            match read {
                Ok(body) => body,
                Err(ureq::Error::BodyExceedsLimit(_)) if limit < per_call => {
                    return Err(Failure::new(
                        FailureRule::OverLimit,
                        format!(
                            "the responses of this update passed its limit of {} bytes",
                            self.limits.max_update_response_bytes
                        ),
                    ));
                }
                Err(ureq::Error::BodyExceedsLimit(_)) => {
                    return Err(Failure::new(
                        FailureRule::OverLimit,
                        format!("the response body is over the limit of {per_call} bytes"),
                    ));
                }
                Err(error) => return Err(self.failure(error, request, deadline)),
            }
        };
        *update_bytes_left = update_bytes_left.saturating_sub(body.len());
        Ok(Answer {
            location,
            content_type,
            response: Response {
                status,
                headers,
                body,
            },
        })
    }

    /// Names the rule behind a `ureq` error.
    fn failure(&self, error: ureq::Error, request: &Request, deadline: Instant) -> Failure {
        let host = request.host();
        match error {
            ureq::Error::Timeout(_) => self.out_of_time(deadline),
            ureq::Error::LargeResponseHeader(_, limit) => Failure::new(
                FailureRule::OverLimit,
                format!("the response headers are over the limit of {limit} bytes"),
            ),
            ureq::Error::BodyExceedsLimit(limit) => Failure::new(
                FailureRule::OverLimit,
                format!("the response is over the limit of {limit} bytes"),
            ),
            ureq::Error::HostNotFound => {
                match self
                    .refused
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .take()
                {
                    Some(reason) => Failure::new(FailureRule::NonPublicAddress, reason),
                    None => Failure::new(
                        FailureRule::Unavailable,
                        format!("{host} was not found ({NO_PROXY_NOTE})"),
                    ),
                }
            }
            other => Failure::new(
                FailureRule::Unavailable,
                format!("{host}: {other} ({NO_PROXY_NOTE})"),
            ),
        }
    }
}

/// One answer, before the host decides whether it is a redirect to follow.
struct Answer {
    location: Option<String>,
    content_type: Option<String>,
    response: Response,
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// The request of a redirect, if the connector allows its target: the scheme, host and port
/// must be exactly one of the connector's origins (research R7). A read becomes a `GET` on 301,
/// 302 and 303 and keeps its method on 307 and 308.
fn follow(
    connector: &Connector,
    request: &Request,
    status: u16,
    location: &str,
) -> Result<Request, Failure> {
    let refuse = |why: String| Failure::new(FailureRule::RedirectNotAllowed, why);
    let target = request.url.join(location).map_err(|error| {
        refuse(format!(
            "to `{location}`, which is not a valid address: {error}"
        ))
    })?;
    let port = target.port_or_known_default();
    let allowed = matches!(target.scheme(), "https" | "http")
        && target.username().is_empty()
        && target.password().is_none()
        && connector.destinations().iter().any(|origin| {
            Some(origin.host()) == target.host_str()
                && Some(origin.port()) == port
                && origin.is_https() == (target.scheme() == "https")
        });
    if !allowed {
        let mut shown = target.clone();
        shown.set_query(None);
        shown.set_fragment(None);
        return Err(refuse(format!(
            "to {shown}, which is not one of the connector's destinations"
        )));
    }
    let keeps_method = matches!(status, 307 | 308);
    let mut target = target;
    target.set_fragment(None);
    Ok(Request {
        method: if keeps_method {
            request.method
        } else {
            Method::Get
        },
        url: target,
        headers: request
            .headers
            .iter()
            .filter(|(name, _)| keeps_method || !name.eq_ignore_ascii_case("content-type"))
            .cloned()
            .collect(),
        body: if keeps_method {
            request.body.clone()
        } else {
            None
        },
    })
}

/// The response, if its media type is the operation's format (`json`: `application/json` or
/// `…+json`; `text`: `text/…` or `…+xml`). Only a success (2xx) with a body is checked: an empty
/// body has no format, and an error status is passed on as it is.
fn check_format(
    format: Format,
    content_type: Option<&str>,
    response: Response,
) -> Result<Response, Failure> {
    // An error status is a normal answer whatever its body is, often an HTML page from a proxy
    // or the service itself: the widget gets its status and decides (edge cases of the spec)
    if response.body.is_empty() || !(200..300).contains(&response.status) {
        return Ok(response);
    }
    let media = content_type
        .and_then(|value| value.split(';').next())
        .map(|media| media.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let matches = match format {
        Format::Json => media == "application/json" || media.ends_with("+json"),
        Format::Text => media.starts_with("text/") || media.ends_with("+xml"),
    };
    if matches {
        Ok(response)
    } else {
        let expected = match format {
            Format::Json => "JSON (application/json or …+json)",
            Format::Text => "text (text/… or …+xml)",
        };
        let found = if media.is_empty() {
            "no media type"
        } else {
            media.as_str()
        };
        Err(Failure::new(
            FailureRule::InvalidResponse,
            format!("the response is {found}, and the operation declares {expected}"),
        ))
    }
}

/// Bytes a request puts on the wire, as logged: the request line, the fixed headers and the body.
fn sent_bytes(request: &Request) -> u64 {
    let line = request.method.as_str().len() + request.target().len();
    let headers: usize = request
        .headers
        .iter()
        .map(|(name, value)| name.len() + value.len())
        .sum();
    (line + headers + request.body.as_ref().map_or(0, Vec::len)) as u64
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::connector::Source;

    fn connector(source: Source, edit: impl FnOnce(&mut Value)) -> Connector {
        let mut connector = json!({
            "connector_version": 1,
            "id": if source == Source::Catalog { "catalog/echo" } else { "local/echo" },
            "version": "1.0.0",
            "author": "Tests",
            "description": "An echo service",
            "destinations": {
                "api": "https://api.echo.test",
                "files": "https://files.echo.test:8443"
            },
            "operations": {
                "get": {
                    "kind": "read", "method": "GET", "destination": "api",
                    "path": "/v1/{id}/items",
                    "query": { "q": "{q}", "page": "{page}", "fixed": "a b&c" },
                    "parameters": {
                        "id": { "type": "text", "max_length": 64 },
                        "q": { "type": "text", "max_length": 64, "required": false },
                        "page": { "type": "integer", "minimum": 1, "maximum": 9, "required": false }
                    },
                    "response": { "format": "json" }
                }
            }
        });
        if source == Source::Catalog {
            connector["reserves"] = json!(["echo.test"]);
        }
        edit(&mut connector);
        Connector::parse(&serde_json::to_vec(&connector).unwrap(), source).unwrap()
    }

    fn text(value: &str) -> ArgValue {
        ArgValue::Text(value.to_owned())
    }

    fn build(connector: &Connector, arguments: &[(&str, ArgValue)]) -> Result<Request, Refusal> {
        let operation = connector.operations().next().unwrap().1;
        let arguments: Vec<(String, ArgValue)> = arguments
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect();
        let checked = operation.check_arguments(&arguments, 16).unwrap();
        build_request(connector, operation, &checked, &Limits::default())
    }

    #[test]
    fn values_stay_inside_their_own_path_segment_or_query_value() {
        let echo = connector(Source::Local, |_| {});
        for (value, encoded) in [
            ("abc", "abc"),
            ("a/b", "a%2Fb"),
            ("a?b", "a%3Fb"),
            ("a#b", "a%23b"),
            ("%2F", "%252F"),
            ("%2e%2e", "%252e%252e"),
            ("...", "..."),
            ("a b", "a%20b"),
            ("é", "%C3%A9"),
            ("a-b_c.d~e", "a-b_c.d~e"),
        ] {
            let request = build(&echo, &[("id", text(value))]).unwrap();
            assert_eq!(
                request.url.path(),
                format!("/v1/{encoded}/items"),
                "{value}"
            );
            assert_eq!(request.url.query(), Some("fixed=a+b%26c"), "{value}");
        }
        let request = build(
            &echo,
            &[
                ("id", text("x")),
                ("q", text("a&b=c#d?e/f")),
                ("page", ArgValue::Integer(3)),
            ],
        )
        .unwrap();
        let pairs: Vec<(String, String)> = request.url.query_pairs().into_owned().collect();
        assert_eq!(
            pairs,
            [
                ("fixed".to_owned(), "a b&c".to_owned()),
                ("page".to_owned(), "3".to_owned()),
                ("q".to_owned(), "a&b=c#d?e/f".to_owned()),
            ]
        );
        assert_eq!(request.url.host_str(), Some("api.echo.test"));
        assert_eq!(request.method, Method::Get);
        assert!(request.body.is_none());
    }

    #[test]
    fn an_absent_optional_parameter_drops_its_query_pair() {
        let echo = connector(Source::Local, |c| {
            c["operations"]["get"]["query"] = json!({ "q": "{q}", "page": "{page}" });
        });
        let request = build(&echo, &[("id", text("x"))]).unwrap();
        assert_eq!(request.url.query(), None);
        assert_eq!(request.target(), "/v1/x/items");
    }

    #[test]
    fn body_values_become_json_values_of_their_type() {
        let echo = connector(Source::Catalog, |c| {
            let operation = &mut c["operations"]["get"];
            operation["method"] = json!("POST");
            operation["query"] = json!({ "fixed": "1" });
            operation["body"] = json!({
                "query": "{q}",
                "page": "{page}",
                "nested": { "list": ["{flag}", "literal"] },
                "fixed": "{not a placeholder}"
            });
            operation["parameters"]["flag"] = json!({ "type": "flag", "required": false });
        });
        let hostile = "a\"b\\c}d\ne\u{1}f";
        let request = build(
            &echo,
            &[
                ("id", text("x")),
                ("q", text(hostile)),
                ("flag", ArgValue::Flag(true)),
            ],
        )
        .unwrap();
        let body: Value = serde_json::from_slice(request.body.as_ref().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({
                "query": hostile,
                "nested": { "list": [true, "literal"] },
                "fixed": "{not a placeholder}"
            })
        );
        assert!(
            request
                .headers
                .contains(&("Content-Type".to_owned(), "application/json".to_owned()))
        );
        let request = build(&echo, &[("id", text("x")), ("page", ArgValue::Integer(7))]).unwrap();
        let body: Value = serde_json::from_slice(request.body.as_ref().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "page": 7, "nested": { "list": ["literal"] }, "fixed": "{not a placeholder}" })
        );
    }

    #[test]
    fn requests_over_the_size_limits_are_refused() {
        let echo = connector(Source::Local, |c| {
            c["operations"]["get"]["parameters"]["q"]["max_length"] = json!(2048);
        });
        let long = "/".repeat(2048);
        let limits = Limits {
            max_request_address_bytes: 4096,
            ..Limits::default()
        };
        let operation = echo.operation("get").unwrap();
        let arguments = operation
            .check_arguments(
                &[("id".to_owned(), text("x")), ("q".to_owned(), text(&long))],
                16,
            )
            .unwrap();
        let Refusal(why) = build_request(&echo, operation, &arguments, &limits).unwrap_err();
        assert!(why.contains("4096"), "{why}");

        let echo = connector(Source::Catalog, |c| {
            let operation = &mut c["operations"]["get"];
            operation["method"] = json!("POST");
            operation["query"] = json!({ "fixed": "1" });
            operation["body"] = json!({ "q": "{q}", "page": "{page}" });
            operation["parameters"]["q"]["max_length"] = json!(2048);
        });
        let limits = Limits {
            max_request_body_bytes: 1024,
            ..Limits::default()
        };
        let operation = echo.operation("get").unwrap();
        let arguments = operation
            .check_arguments(
                &[("id".to_owned(), text("x")), ("q".to_owned(), text(&long))],
                16,
            )
            .unwrap();
        let Refusal(why) = build_request(&echo, operation, &arguments, &limits).unwrap_err();
        assert!(why.contains("1024"), "{why}");
    }

    fn request(url: &str, method: Method) -> Request {
        Request {
            method,
            url: url::Url::parse(url).unwrap(),
            headers: vec![("Accept".to_owned(), "application/json".to_owned())],
            body: Some(b"{}".to_vec()),
        }
    }

    #[test]
    fn redirects_are_followed_only_to_the_connectors_own_origins() {
        let echo = connector(Source::Local, |_| {});
        let from = request("https://api.echo.test/v1/a", Method::Post);
        for (location, url) in [
            ("/v2/b", "https://api.echo.test/v2/b"),
            ("b?x=1", "https://api.echo.test/v1/b?x=1"),
            ("https://API.echo.test/c#top", "https://api.echo.test/c"),
            (
                "https://files.echo.test:8443/f",
                "https://files.echo.test:8443/f",
            ),
        ] {
            let next = follow(&echo, &from, 302, location).unwrap();
            assert_eq!(next.url.as_str(), url, "{location}");
            assert_eq!(next.method, Method::Get);
            assert!(next.body.is_none());
        }
        for location in [
            "https://elsewhere.test/",
            "https://api.echo.test:8443/",
            "https://files.echo.test/",
            "http://api.echo.test/",
            "https://ada@api.echo.test/",
            "https://10.0.0.1/",
            "ftp://api.echo.test/",
            "https://[::1]/",
        ] {
            let failure = follow(&echo, &from, 302, location).unwrap_err();
            assert_eq!(failure.rule, FailureRule::RedirectNotAllowed, "{location}");
        }
        // 307 and 308 keep the method and the body
        let next = follow(&echo, &from, 307, "/again").unwrap();
        assert_eq!(next.method, Method::Post);
        assert_eq!(next.body.as_deref(), Some(&b"{}"[..]));
    }

    #[test]
    fn a_response_must_have_the_declared_format() {
        let response = |body: &str| Response {
            status: 200,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        };
        for media in [
            "application/json",
            "application/json; charset=utf-8",
            "application/vnd.github+json",
            "APPLICATION/JSON",
        ] {
            assert!(
                check_format(Format::Json, Some(media), response("{}")).is_ok(),
                "{media}"
            );
        }
        for media in [
            None,
            Some("text/html"),
            Some("application/javascript"),
            Some(""),
        ] {
            let failure = check_format(Format::Json, media, response("{}")).unwrap_err();
            assert_eq!(failure.rule, FailureRule::InvalidResponse, "{media:?}");
        }
        for media in [
            "text/plain",
            "text/html; charset=utf-8",
            "application/rss+xml",
        ] {
            assert!(
                check_format(Format::Text, Some(media), response("x")).is_ok(),
                "{media}"
            );
        }
        assert!(check_format(Format::Text, Some("application/json"), response("x")).is_err());
        // An empty body has no format to check
        assert!(check_format(Format::Json, Some("text/html"), response("")).is_ok());
        // An error status passes on whatever its body is
        for status in [301, 404, 500, 502, 503] {
            let page = Response {
                status,
                headers: Vec::new(),
                body: b"<html>Bad gateway</html>".to_vec(),
            };
            let passed = check_format(Format::Json, Some("text/html"), page).unwrap();
            assert_eq!(passed.status, status);
            assert_eq!(passed.body, b"<html>Bad gateway</html>");
        }
    }

    #[test]
    fn the_logged_target_is_path_and_query() {
        let request = request("https://api.echo.test/v1/a?b=1&c=2", Method::Get);
        assert_eq!(request.target(), "/v1/a?b=1&c=2");
        assert_eq!(request.host(), "api.echo.test");
        assert!(sent_bytes(&request) > 0);
    }
}
