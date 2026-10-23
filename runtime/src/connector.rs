//! Connectors: declarations, never code, of the operations a widget may call on one external
//! service, which the host runs (`specs/003-plugin-connectors/data-model.md`, Connector; research
//! R1, R3, R4).
//!
//! A connector is untrusted input until it is checked: a local one is any file in the
//! development directory, and even a catalog one passes the same checks. It is parsed into typed
//! structs, so nothing outside the format is accepted, then checked against every rule of
//! `wit/connector.schema.json` and the rules only the host can check, such as the fixed headers
//! a connector outside the catalog may not set. `tests/manifest_schema.rs` keeps the schema and
//! this parser in agreement.

use std::collections::BTreeMap;
use std::fmt;

use crate::error::{ErrorKind, RuntimeError};

/// Largest connector file accepted, checked before parsing.
pub(crate) const MAX_CONNECTOR_BYTES: usize = 64 * 1024;

/// Where a connector comes from, which decides the rules it follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// Built into the host from `connectors/catalog/`, reviewed line by line: it lists the
    /// domains it reserves, may fix `X-` headers, and may declare a read with a body.
    Catalog,
    /// A file of the development directory: reads are body-less `GET` or `HEAD` requests, no
    /// `X-` header, and its destinations may be this machine over plain HTTP.
    Local,
    /// A connector from a repository: parsed so the refusal can say why, refused at load until
    /// installation verifies its destinations (Phase 3).
    Repository,
}

/// A connector, as checked by [`Connector::parse`]; there is no other way to build one.
#[derive(Debug, Clone)]
pub struct Connector {
    id: String,
    version: semver::Version,
    author: String,
    description: String,
    reserves: Vec<String>,
    destinations: BTreeMap<String, Origin>,
    operations: BTreeMap<String, Operation>,
}

/// Where requests go: a scheme, a host in its normal form, and a port.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Origin {
    https: bool,
    host: String,
    port: u16,
}

/// One request a connector can make.
#[derive(Debug, Clone)]
pub struct Operation {
    kind: Kind,
    method: Method,
    destination: String,
    path: Vec<Template>,
    query: Vec<(String, Template)>,
    headers: Vec<(String, String)>,
    body: Option<serde_json::Value>,
    parameters: BTreeMap<String, Parameter>,
    response: ResponseSpec,
    /// The operation as written, for its digest.
    raw: serde_json::Value,
}

/// Whether an operation changes data in its service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Changes nothing in the service; it still sends its parameters.
    Read,
    /// Changes data in the service.
    Change,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

/// A path segment, a query value or a body string: literal, or exactly one parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Template {
    Literal(String),
    Parameter(String),
}

/// A value a widget fills in, with the type the host checks on every call.
#[derive(Debug, Clone)]
pub struct Parameter {
    pub kind: ParameterType,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub enum ParameterType {
    Text {
        max_length: usize,
        /// The pattern as written, and compiled to match a whole value.
        pattern: Option<(String, regex::Regex)>,
        values: Option<Vec<String>>,
    },
    Integer {
        minimum: i64,
        maximum: i64,
    },
    Flag,
}

/// What the widget receives of a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseSpec {
    pub format: Format,
    /// The response headers passed to the widget, in the order declared.
    pub headers: Vec<String>,
}

/// The media type a response must have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `application/json` or `…+json`.
    Json,
    /// `text/…` or `…+xml`.
    Text,
}

/// A value a widget passes in a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgValue {
    Text(String),
    Integer(i64),
    Flag(bool),
}

impl Connector {
    /// Parses and checks a connector file. Refuses it as `invalid-connector`, naming the field
    /// path and the problem, or as `credentials-not-supported` when it declares a credential.
    pub(crate) fn parse(bytes: &[u8], source: Source) -> Result<Connector, RuntimeError> {
        let _ = (bytes, source);
        Err(RuntimeError::new(
            ErrorKind::InvalidConnector,
            "not implemented",
        ))
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn version(&self) -> &semver::Version {
        &self.version
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// The domains a catalog connector reserves; empty for every other connector.
    pub fn reserves(&self) -> &[String] {
        &self.reserves
    }

    /// Every origin the connector declares, sorted, each once.
    pub fn destinations(&self) -> Vec<&Origin> {
        let mut origins: Vec<&Origin> = self.destinations.values().collect();
        origins.sort();
        origins.dedup();
        origins
    }

    /// The origin of the destination named `name`.
    pub fn destination(&self, name: &str) -> Option<&Origin> {
        self.destinations.get(name)
    }

    /// Every operation, by name, in name order.
    pub fn operations(&self) -> impl Iterator<Item = (&str, &Operation)> {
        self.operations
            .iter()
            .map(|(name, operation)| (name.as_str(), operation))
    }

    pub fn operation(&self, name: &str) -> Option<&Operation> {
        self.operations.get(name)
    }

    /// `sha256:` and the hex SHA-256 of the operation's canonical declaration, with its
    /// destination replaced by the destination's origin: it changes when any field of the
    /// operation or that origin changes, and not when another operation is added (research R13).
    pub fn operation_digest(&self, name: &str) -> Option<String> {
        let _ = name;
        None
    }
}

impl Origin {
    pub fn is_https(&self) -> bool {
        self.https
    }

    /// The host in its normal form: lowercase, internationalized labels as `xn--`, or a
    /// loopback address (`127.0.0.1`, `[::1]`) for a local connector.
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// `https://api.github.com`, `http://127.0.0.1:8765`: the port only when it is not the
    /// scheme's default.
    pub fn serialization(&self) -> String {
        let (scheme, default) = if self.https {
            ("https", 443)
        } else {
            ("http", 80)
        };
        if self.port == default {
            format!("{scheme}://{}", self.host)
        } else {
            format!("{scheme}://{}:{}", self.host, self.port)
        }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.serialization())
    }
}

impl Operation {
    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn method(&self) -> Method {
        self.method
    }

    /// The name of its destination, a key of the connector's `destinations`.
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// The path's segments, after the leading `/`.
    pub fn path(&self) -> &[Template] {
        &self.path
    }

    pub fn query(&self) -> &[(String, Template)] {
        &self.query
    }

    /// The fixed request headers, in name order.
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    pub fn body(&self) -> Option<&serde_json::Value> {
        self.body.as_ref()
    }

    pub fn parameters(&self) -> &BTreeMap<String, Parameter> {
        &self.parameters
    }

    pub fn response(&self) -> &ResponseSpec {
        &self.response
    }

    /// Checks a call's arguments against the parameters, before anything is sent: at most
    /// `max_arguments`, each naming a declared parameter once, every required one present, each
    /// value of its type and within its bounds; in a path segment, never `""`, `.` or `..`. The
    /// error names the parameter and the rule (`value-outside-type`).
    pub fn check_arguments(
        &self,
        arguments: &[(String, ArgValue)],
        max_arguments: usize,
    ) -> Result<BTreeMap<String, ArgValue>, String> {
        let _ = (arguments, max_arguments);
        Err("not implemented".to_owned())
    }
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Read => "read",
            Kind::Change => "change",
        }
    }
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    /// A valid local connector, changed by `edit`.
    fn local_with(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut connector = json!({
            "$schema": "../../wit/connector.schema.json",
            "connector_version": 1,
            "id": "local/weather",
            "version": "0.1.0",
            "author": "Ada Lovelace",
            "description": "Weather forecasts",
            "destinations": { "api": "https://api.weather.test" },
            "operations": {
                "forecast": {
                    "kind": "read",
                    "method": "GET",
                    "destination": "api",
                    "path": "/v1/{city}/forecast",
                    "query": { "days": "{days}", "units": "metric" },
                    "headers": { "Accept": "application/json" },
                    "parameters": {
                        "city": { "type": "text", "max_length": 64, "pattern": "[a-z]+" },
                        "days": { "type": "integer", "minimum": 1, "maximum": 7, "required": false }
                    },
                    "response": { "format": "json", "headers": ["ETag"] }
                }
            }
        });
        edit(&mut connector);
        serde_json::to_vec(&connector).unwrap()
    }

    /// The same connector as a catalog one, reserving its domain.
    fn catalog_with(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
        local_with(|connector| {
            connector["id"] = json!("catalog/weather");
            connector["reserves"] = json!(["weather.test"]);
            edit(connector);
        })
    }

    fn forecast(connector: &mut Value) -> &mut Value {
        &mut connector["operations"]["forecast"]
    }

    fn parse_local(edit: impl FnOnce(&mut Value)) -> Result<Connector, RuntimeError> {
        Connector::parse(&local_with(edit), Source::Local)
    }

    fn parse_catalog(edit: impl FnOnce(&mut Value)) -> Result<Connector, RuntimeError> {
        Connector::parse(&catalog_with(edit), Source::Catalog)
    }

    /// Asserts `result` is an `invalid-connector` error whose detail contains each of `parts`.
    #[track_caller]
    fn assert_invalid(result: Result<Connector, RuntimeError>, parts: &[&str]) {
        let error = result.expect_err("the connector should be refused");
        assert_eq!(error.kind(), ErrorKind::InvalidConnector, "{error}");
        for part in parts {
            assert!(
                error.detail().contains(part),
                "{error} should mention `{part}`"
            );
        }
    }

    #[track_caller]
    fn assert_invalid_local(edit: impl FnOnce(&mut Value), parts: &[&str]) {
        assert_invalid(parse_local(edit), parts);
    }

    #[test]
    fn a_valid_local_connector_parses() {
        let connector = parse_local(|_| {}).unwrap();
        assert_eq!(connector.id(), "local/weather");
        assert_eq!(connector.version(), &semver::Version::new(0, 1, 0));
        assert_eq!(connector.author(), "Ada Lovelace");
        assert_eq!(connector.description(), "Weather forecasts");
        assert!(connector.reserves().is_empty());
        let origins: Vec<String> = connector
            .destinations()
            .iter()
            .map(|origin| origin.serialization())
            .collect();
        assert_eq!(origins, ["https://api.weather.test"]);

        let operation = connector.operation("forecast").unwrap();
        assert_eq!(operation.kind(), Kind::Read);
        assert_eq!(operation.method(), Method::Get);
        assert_eq!(operation.destination(), "api");
        assert_eq!(
            operation.path(),
            [
                Template::Literal("v1".to_owned()),
                Template::Parameter("city".to_owned()),
                Template::Literal("forecast".to_owned()),
            ]
        );
        assert_eq!(
            operation.query(),
            [
                ("days".to_owned(), Template::Parameter("days".to_owned())),
                ("units".to_owned(), Template::Literal("metric".to_owned())),
            ]
        );
        assert_eq!(
            operation.headers(),
            [("Accept".to_owned(), "application/json".to_owned())]
        );
        assert!(operation.body().is_none());
        assert!(operation.parameters()["city"].required);
        assert!(!operation.parameters()["days"].required);
        assert_eq!(
            operation.response(),
            &ResponseSpec {
                format: Format::Json,
                headers: vec!["ETag".to_owned()],
            }
        );
    }

    #[test]
    fn the_catalog_connector_of_the_repository_parses() {
        let connector = Connector::parse(
            include_bytes!("../../connectors/catalog/github.json"),
            Source::Catalog,
        )
        .unwrap();
        assert_eq!(connector.id(), "catalog/github");
        assert_eq!(connector.reserves(), ["github.com"]);
        assert!(connector.operation("list-releases").is_some());
    }

    #[test]
    fn size_encoding_and_document_rules() {
        let large = local_with(|connector| {
            connector["description"] = json!("x".repeat(MAX_CONNECTOR_BYTES));
        });
        assert_invalid(Connector::parse(&large, Source::Local), &["65536"]);
        assert_invalid(Connector::parse(b"\xff", Source::Local), &["UTF-8"]);
        assert_invalid(Connector::parse(b"[1]", Source::Local), &[]);
        assert_invalid(Connector::parse(b"{} {}", Source::Local), &[]);
        assert_invalid_local(
            |c| c["connector_version"] = json!(2),
            &["connector_version"],
        );
        assert_invalid_local(|c| c["colour"] = json!("blue"), &["colour"]);
        assert_invalid_local(|c| forecast(c)["timeout"] = json!(5), &["timeout"]);
        // $schema is for editors and ignored
        assert!(parse_local(|c| c["$schema"] = json!("anything")).is_ok());
    }

    #[test]
    fn a_key_given_twice_is_refused() {
        let text = String::from_utf8(local_with(|_| {})).unwrap().replacen(
            r#""author":"Ada Lovelace""#,
            r#""author":"Ada Lovelace","author":"Eve""#,
            1,
        );
        assert_invalid(
            Connector::parse(text.as_bytes(), Source::Local),
            &["author"],
        );
        let text = String::from_utf8(local_with(|_| {})).unwrap().replacen(
            r#""units":"metric""#,
            r#""units":"metric","units":"imperial""#,
            1,
        );
        assert_invalid(Connector::parse(text.as_bytes(), Source::Local), &["units"]);
    }

    #[test]
    fn id_and_version_rules() {
        for id in [
            "local/",
            "local/Weather",
            "local/-weather",
            "local/weather-",
            "catalog/Weather",
            "other/weather",
            "github.com/ada",
        ] {
            assert_invalid_local(|c| c["id"] = json!(id), &["id"]);
        }
        let long = format!("local/{}", "a".repeat(64));
        assert_invalid_local(|c| c["id"] = json!(long), &["id"]);
        assert_invalid(
            Connector::parse(&local_with(|_| {}), Source::Catalog),
            &["id"],
        );
        // A connector outside the catalog that claims a catalog id passes for one
        let impostor = parse_local(|c| c["id"] = json!("catalog/weather")).unwrap_err();
        assert_eq!(
            impostor.kind(),
            ErrorKind::DestinationReserved,
            "{impostor}"
        );
        assert!(impostor.detail().contains("catalog/weather"), "{impostor}");
        let repository = local_with(|c| c["id"] = json!("github.com/ada/weather"));
        assert_eq!(
            Connector::parse(&repository, Source::Repository)
                .unwrap()
                .id(),
            "github.com/ada/weather"
        );
        assert_invalid(Connector::parse(&repository, Source::Local), &["id"]);
        assert_invalid_local(|c| c["version"] = json!("1.0"), &["version"]);
        assert_invalid_local(
            |c| c["version"] = json!(format!("1.0.0-{}", "a".repeat(60))),
            &["version", "64"],
        );
    }

    #[test]
    fn author_and_description_rules() {
        assert_invalid_local(|c| c["author"] = json!(""), &["author"]);
        assert_invalid_local(|c| c["author"] = json!("a".repeat(101)), &["author"]);
        assert_invalid_local(
            |c| c["author"] = json!("Ada\nLovelace"),
            &["author", "control"],
        );
        assert_invalid_local(|c| c["description"] = json!(""), &["description"]);
        assert_invalid_local(
            |c| c["description"] = json!("d".repeat(201)),
            &["description"],
        );
        assert_invalid_local(
            |c| c["description"] = json!("bad \u{1b}[2J"),
            &["description", "control"],
        );
        assert!(parse_local(|c| c["author"] = json!("a".repeat(100))).is_ok());
    }

    #[test]
    fn reserves_belong_to_catalog_connectors_and_cover_their_hosts() {
        assert!(parse_catalog(|_| {}).is_ok());
        assert_invalid_local(|c| c["reserves"] = json!(["weather.test"]), &["reserves"]);
        assert_invalid(
            parse_catalog(|c| {
                c.as_object_mut().unwrap().remove("reserves");
            }),
            &["reserves"],
        );
        assert_invalid(parse_catalog(|c| c["reserves"] = json!([])), &["reserves"]);
        let nine: Vec<String> = (0..9).map(|i| format!("d{i}.test")).collect();
        assert_invalid(
            parse_catalog(|c| c["reserves"] = json!(nine)),
            &["reserves"],
        );
        for domain in [
            "Weather.test",
            "test",
            "weather.test.",
            "-weather.test",
            "weather.1",
        ] {
            assert_invalid(
                parse_catalog(|c| c["reserves"] = json!([domain])),
                &["reserves"],
            );
        }
        assert_invalid(
            parse_catalog(|c| c["reserves"] = json!(["other.test"])),
            &["api.weather.test"],
        );
        // A domain reserves itself and every host under it, and only those
        assert!(
            parse_catalog(|c| {
                c["destinations"]["root"] = json!("https://weather.test");
            })
            .is_ok()
        );
        assert_invalid(
            parse_catalog(|c| c["reserves"] = json!(["eather.test"])),
            &["api.weather.test"],
        );
    }

    #[test]
    fn destination_rules() {
        assert_invalid_local(|c| c["destinations"] = json!({}), &["destinations"]);
        let nine: serde_json::Map<String, Value> = (0..9)
            .map(|i| (format!("d{i}"), json!(format!("https://d{i}.test"))))
            .collect();
        assert_invalid_local(
            |c| c["destinations"] = Value::Object(nine),
            &["destinations"],
        );
        assert_invalid_local(
            |c| c["destinations"]["Bad"] = json!("https://x.test"),
            &["destinations", "Bad"],
        );
        for origin in [
            "https://api.weather.test/v1",
            "https://api.weather.test?x=1",
            "https://api.weather.test#top",
            "https://ada@api.weather.test",
            "https://ada:pw@api.weather.test",
            "https://api.weather.test.",
            "ftp://api.weather.test",
            "api.weather.test",
            "https://10.0.0.1",
            "https://[2001:db8::1]",
            "https://127.0.0.1:8443",
            "https://api.weather.test:0",
            " https://api.weather.test",
            "http://api.weather.test",
            "http://localhost",
            "http://192.168.1.10:8765",
        ] {
            assert_invalid_local(
                |c| c["destinations"]["api"] = json!(origin),
                &["destinations.api"],
            );
        }
        let long = format!("https://{}.test", "a".repeat(260));
        assert_invalid_local(
            |c| c["destinations"]["api"] = json!(long),
            &["destinations.api"],
        );
        // The root path is no path
        assert!(
            parse_local(|c| c["destinations"]["api"] = json!("https://api.weather.test/")).is_ok()
        );
    }

    #[test]
    fn hosts_are_compared_in_one_normal_form() {
        let connector = parse_local(|c| {
            c["destinations"]["api"] = json!("HTTPS://API.Weather.TEST:443");
        })
        .unwrap();
        let origin = connector.destination("api").unwrap();
        assert_eq!(origin.host(), "api.weather.test");
        assert_eq!(origin.port(), 443);
        assert!(origin.is_https());
        assert_eq!(origin.serialization(), "https://api.weather.test");

        let connector =
            parse_local(|c| c["destinations"]["api"] = json!("https://bücher.test:8443")).unwrap();
        assert_eq!(
            connector.destination("api").unwrap().serialization(),
            "https://xn--bcher-kva.test:8443"
        );
    }

    #[test]
    fn loopback_over_plain_http_only_for_local_connectors() {
        for (origin, host) in [
            ("http://localhost:8765", "localhost"),
            ("http://127.0.0.1:8765", "127.0.0.1"),
            ("http://[::1]:8765", "[::1]"),
        ] {
            let connector = parse_local(|c| c["destinations"]["api"] = json!(origin)).unwrap();
            let destination = connector.destination("api").unwrap();
            assert!(!destination.is_https());
            assert_eq!(destination.host(), host);
            assert_eq!(destination.port(), 8765);
            assert_eq!(destination.serialization(), origin);

            assert_invalid(
                parse_catalog(|c| c["destinations"]["api"] = json!(origin)),
                &["destinations.api"],
            );
            let repository = local_with(|c| {
                c["id"] = json!("github.com/ada/weather");
                c["destinations"]["api"] = json!(origin);
            });
            assert_invalid(
                Connector::parse(&repository, Source::Repository),
                &["destinations.api"],
            );
        }
    }

    #[test]
    fn operation_rules() {
        assert_invalid_local(|c| c["operations"] = json!({}), &["operations"]);
        let many: serde_json::Map<String, Value> = (0..65)
            .map(|i| {
                let mut operation = json!({
                    "kind": "read", "method": "GET", "destination": "api", "path": "/",
                    "response": { "format": "json" }
                });
                operation["path"] = json!(format!("/o{i}"));
                (format!("o{i}"), operation)
            })
            .collect();
        assert_invalid_local(|c| c["operations"] = Value::Object(many), &["operations"]);
        assert_invalid_local(
            |c| {
                let operation = forecast(c).clone();
                c["operations"]["Forecast"] = operation;
            },
            &["operations", "Forecast"],
        );
        assert_invalid_local(
            |c| forecast(c)["kind"] = json!("write"),
            &["operations.forecast.kind"],
        );
        assert_invalid_local(
            |c| forecast(c)["method"] = json!("get"),
            &["operations.forecast.method"],
        );
        assert_invalid_local(
            |c| forecast(c)["method"] = json!("OPTIONS"),
            &["operations.forecast.method"],
        );
        assert_invalid_local(
            |c| forecast(c)["destination"] = json!("elsewhere"),
            &["operations.forecast.destination", "elsewhere"],
        );
        for required in ["kind", "method", "destination", "path", "response"] {
            assert_invalid_local(
                |c| {
                    forecast(c).as_object_mut().unwrap().remove(required);
                },
                &[required],
            );
        }
    }

    #[test]
    fn outside_the_catalog_a_read_is_a_get_or_head_without_a_body() {
        for method in ["POST", "PUT", "PATCH", "DELETE"] {
            assert_invalid_local(
                |c| forecast(c)["method"] = json!(method),
                &["operations.forecast", "change"],
            );
        }
        assert_invalid_local(
            |c| forecast(c)["body"] = json!({ "city": "{city}" }),
            &["operations.forecast", "change"],
        );
        assert!(parse_local(|c| forecast(c)["method"] = json!("HEAD")).is_ok());
        // Declared as a change, the same request is fine
        assert!(
            parse_local(|c| {
                forecast(c)["kind"] = json!("change");
                forecast(c)["method"] = json!("POST");
                forecast(c)["body"] = json!({ "note": "fixed" });
            })
            .is_ok()
        );
        // The catalog's review checks the kind, so it may declare a read with a body
        assert!(
            parse_catalog(|c| {
                forecast(c)["method"] = json!("POST");
                forecast(c)["body"] = json!({ "query": "{city}" });
                forecast(c)["path"] = json!("/graphql");
            })
            .is_ok()
        );
    }

    #[test]
    fn path_rules() {
        for path in [
            "v1/{city}",
            "/v1/{city}x",
            "/v1/x{city}",
            "/v1/{City}",
            "/v1?x/{city}",
            "/v1#x/{city}",
            "/v1/{/{city}",
            "/v1/}/{city}",
            "/v1/a b/{city}",
            "/v1/é/{city}",
        ] {
            assert_invalid_local(
                |c| forecast(c)["path"] = json!(path),
                &["operations.forecast.path"],
            );
        }
        let long = format!("/{}/{{city}}", "a".repeat(512));
        assert_invalid_local(
            |c| forecast(c)["path"] = json!(long),
            &["operations.forecast.path"],
        );
        assert!(
            parse_local(|c| forecast(c)["path"] = json!("/a.b_c~d!$&'()*+,;=:@%-/{city}")).is_ok()
        );
    }

    #[test]
    fn query_rules() {
        let many: serde_json::Map<String, Value> =
            (0..17).map(|i| (format!("q{i}"), json!("x"))).collect();
        assert_invalid_local(
            |c| {
                let mut query = many.clone();
                query.insert("days".to_owned(), json!("{days}"));
                forecast(c)["query"] = Value::Object(query);
            },
            &["operations.forecast.query"],
        );
        for name in ["", "a b", "a&b", "a=b", &"q".repeat(65)] {
            assert_invalid_local(
                |c| forecast(c)["query"][name] = json!("x"),
                &["operations.forecast.query"],
            );
        }
        assert_invalid_local(
            |c| forecast(c)["query"]["units"] = json!("m".repeat(257)),
            &["operations.forecast.query.units"],
        );
    }

    #[test]
    fn fixed_header_rules() {
        assert_invalid_local(
            |c| forecast(c)["headers"]["X-HTTP-Method-Override"] = json!("DELETE"),
            &["operations.forecast.headers", "X-HTTP-Method-Override"],
        );
        assert_invalid_local(
            |c| forecast(c)["headers"]["X-Api-Version"] = json!("2"),
            &["X-Api-Version"],
        );
        assert!(parse_catalog(|c| forecast(c)["headers"]["X-Api-Version"] = json!("2")).is_ok());
        for name in [
            "Authorization",
            "Cookie",
            "Proxy-Authorization",
            "Host",
            "Content-Type",
            "Content-Length",
            "Transfer-Encoding",
            "Connection",
            "Upgrade",
            "TE",
            "User-Agent",
            "accept",
        ] {
            assert_invalid(
                parse_catalog(|c| forecast(c)["headers"][name] = json!("x")),
                &["operations.forecast.headers", name],
            );
        }
        assert_invalid_local(
            |c| forecast(c)["headers"]["Accept"] = json!("text/plain\r\nX-Evil: 1"),
            &["operations.forecast.headers.Accept"],
        );
        assert_invalid_local(
            |c| forecast(c)["headers"]["Accept"] = json!("a".repeat(257)),
            &["operations.forecast.headers.Accept"],
        );
        let many: serde_json::Map<String, Value> =
            (0..9).map(|i| (format!("X-H{i}"), json!("x"))).collect();
        assert_invalid(
            parse_catalog(|c| forecast(c)["headers"] = Value::Object(many)),
            &["operations.forecast.headers"],
        );
    }

    #[test]
    fn body_rules() {
        assert_invalid_local(
            |c| {
                forecast(c)["kind"] = json!("change");
                forecast(c)["method"] = json!("POST");
                forecast(c)["body"] = json!({ "note": "n".repeat(4096) });
            },
            &["operations.forecast.body"],
        );
        assert_invalid_local(
            |c| {
                forecast(c)["kind"] = json!("change");
                forecast(c)["method"] = json!("POST");
                forecast(c)["body"] = json!(["not", "an", "object"]);
            },
            &["operations.forecast.body"],
        );
    }

    #[test]
    fn parameter_rules() {
        let many: serde_json::Map<String, Value> = (0..17)
            .map(|i| (format!("p{i}"), json!({ "type": "flag" })))
            .collect();
        assert_invalid_local(
            |c| forecast(c)["parameters"] = Value::Object(many),
            &["operations.forecast.parameters"],
        );
        let city = |c: &mut Value, parameter: Value| forecast(c)["parameters"]["city"] = parameter;
        for parameter in [
            json!({ "max_length": 64 }),
            json!({ "type": "text" }),
            json!({ "type": "text", "max_length": 0 }),
            json!({ "type": "text", "max_length": 2049 }),
            json!({ "type": "text", "max_length": 8, "pattern": "[a-z]+", "values": ["a"] }),
            json!({ "type": "text", "max_length": 8, "pattern": "" }),
            json!({ "type": "text", "max_length": 8, "pattern": "(" }),
            json!({ "type": "text", "max_length": 8, "pattern": "a".repeat(513) }),
            json!({ "type": "text", "max_length": 8, "pattern": "a{1000}{1000}" }),
            json!({ "type": "text", "max_length": 8, "values": [] }),
            json!({ "type": "text", "max_length": 8, "values": ["a", "a"] }),
            json!({ "type": "text", "max_length": 8, "values": (0..65).map(|i| i.to_string()).collect::<Vec<_>>() }),
            json!({ "type": "text", "max_length": 8, "values": ["v".repeat(2049)] }),
            json!({ "type": "text", "max_length": 8, "minimum": 1 }),
            json!({ "type": "integer", "minimum": 1 }),
            json!({ "type": "integer", "minimum": 8, "maximum": 1 }),
            json!({ "type": "integer", "minimum": 1.5, "maximum": 8 }),
            json!({ "type": "flag", "max_length": 8 }),
            json!({ "type": "date" }),
            json!({ "type": "flag", "required": "yes" }),
        ] {
            assert_invalid_local(
                |c| city(c, parameter.clone()),
                &["operations.forecast.parameters.city"],
            );
        }
        assert_invalid_local(
            |c| {
                forecast(c)["parameters"]["City"] = json!({ "type": "flag" });
            },
            &["operations.forecast.parameters", "City"],
        );
        assert!(
            parse_local(|c| city(
                c,
                json!({ "type": "text", "max_length": 8, "values": ["paris", "rome"] })
            ))
            .is_ok()
        );
        assert!(
            parse_local(|c| city(
                c,
                json!({ "type": "integer", "minimum": i64::MIN, "maximum": i64::MAX })
            ))
            .is_ok()
        );
        assert!(parse_local(|c| city(c, json!({ "type": "flag" }))).is_ok());
    }

    #[test]
    fn placeholder_rules() {
        // Every placeholder names a declared parameter
        assert_invalid_local(
            |c| forecast(c)["path"] = json!("/v1/{town}/forecast"),
            &["operations.forecast", "town"],
        );
        assert_invalid_local(
            |c| forecast(c)["query"]["units"] = json!("{units}"),
            &["operations.forecast", "units"],
        );
        // Every parameter appears in exactly one place
        assert_invalid_local(
            |c| {
                forecast(c)["parameters"]["unused"] = json!({ "type": "flag" });
            },
            &["operations.forecast", "unused"],
        );
        assert_invalid_local(
            |c| forecast(c)["query"]["town"] = json!("{city}"),
            &["operations.forecast", "city"],
        );
        // An optional parameter never fills a path segment
        assert_invalid_local(
            |c| forecast(c)["parameters"]["city"]["required"] = json!(false),
            &["operations.forecast", "city"],
        );
        // In a body, at any depth
        assert!(
            parse_local(|c| {
                forecast(c)["kind"] = json!("change");
                forecast(c)["method"] = json!("POST");
                forecast(c)["query"] = json!({ "units": "metric" });
                forecast(c)["body"] = json!({ "where": { "days": ["{days}"] } });
            })
            .is_ok()
        );
        assert_invalid_local(
            |c| {
                forecast(c)["kind"] = json!("change");
                forecast(c)["method"] = json!("POST");
                forecast(c)["body"] = json!({ "where": { "days": "{days}" } });
            },
            &["operations.forecast", "days"],
        );
    }

    #[test]
    fn response_rules() {
        assert_invalid_local(
            |c| forecast(c)["response"]["format"] = json!("xml"),
            &["operations.forecast.response.format"],
        );
        for name in [
            "Set-Cookie",
            "set-cookie",
            "Cookie",
            "COOKIE",
            "Bad Name",
            "",
        ] {
            assert_invalid_local(
                |c| forecast(c)["response"]["headers"] = json!([name]),
                &["operations.forecast.response.headers"],
            );
        }
        assert_invalid_local(
            |c| forecast(c)["response"]["headers"] = json!(["ETag", "ETag"]),
            &["operations.forecast.response.headers"],
        );
        let many: Vec<String> = (0..9).map(|i| format!("X-H{i}")).collect();
        assert_invalid_local(
            |c| forecast(c)["response"]["headers"] = json!(many),
            &["operations.forecast.response.headers"],
        );
        assert!(parse_local(|c| forecast(c)["response"] = json!({ "format": "text" })).is_ok());
    }

    #[test]
    fn a_credential_anywhere_is_refused_as_not_supported_yet() {
        let edits: [fn(&mut Value); 4] = [
            |c| c["auth"] = json!({ "type": "bearer" }),
            |c| c["credentials"] = json!([]),
            |c| forecast(c)["credential"] = json!("api-key"),
            |c| {
                forecast(c)["kind"] = json!("change");
                forecast(c)["method"] = json!("POST");
                forecast(c)["body"] = json!({ "nested": { "auth": "x" } });
            },
        ];
        for edit in edits {
            let error = parse_local(edit).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::CredentialsNotSupported, "{error}");
            assert!(error.detail().contains("005"), "{error}");
        }
    }

    #[test]
    fn the_digest_of_an_operation_follows_its_declaration_and_destination() {
        let digest = |edit: fn(&mut Value)| {
            parse_local(edit)
                .unwrap()
                .operation_digest("forecast")
                .unwrap()
        };
        let base = digest(|_| {});
        assert!(base.starts_with("sha256:"));
        assert_eq!(base.len(), "sha256:".len() + 64);
        assert_eq!(base, digest(|_| {}));
        // Another operation, or another destination it does not use, changes nothing
        assert_eq!(
            base,
            digest(|c| {
                let mut other = c["operations"]["forecast"].clone();
                other["path"] = json!("/v1/{city}/now");
                c["operations"]["now"] = other;
                c["destinations"]["cdn"] = json!("https://cdn.weather.test");
            })
        );
        // Renaming the destination keeps its origin
        assert_eq!(
            base,
            digest(|c| {
                c["destinations"] = json!({ "main": "https://api.weather.test" });
                forecast(c)["destination"] = json!("main");
            })
        );
        for edit in [
            (|c| c["destinations"]["api"] = json!("https://api2.weather.test")) as fn(&mut Value),
            |c| c["destinations"]["api"] = json!("https://api.weather.test:8443"),
            |c| forecast(c)["method"] = json!("HEAD"),
            |c| forecast(c)["path"] = json!("/v2/{city}/forecast"),
            |c| forecast(c)["query"]["units"] = json!("imperial"),
            |c| forecast(c)["headers"]["Accept"] = json!("text/json"),
            |c| forecast(c)["parameters"]["city"]["max_length"] = json!(65),
            |c| forecast(c)["response"]["headers"] = json!([]),
            |c| {
                forecast(c)["kind"] = json!("change");
            },
        ] {
            assert_ne!(base, digest(edit));
        }
        assert!(
            parse_local(|_| {})
                .unwrap()
                .operation_digest("missing")
                .is_none()
        );
    }

    fn text(value: &str) -> ArgValue {
        ArgValue::Text(value.to_owned())
    }

    fn check(arguments: &[(&str, ArgValue)]) -> Result<BTreeMap<String, ArgValue>, String> {
        let connector = parse_local(|c| {
            forecast(c)["query"]["lang"] = json!("{lang}");
            forecast(c)["query"]["metric"] = json!("{metric}");
            forecast(c)["parameters"]["lang"] =
                json!({ "type": "text", "max_length": 2, "values": ["en", "pt"], "required": false });
            forecast(c)["parameters"]["metric"] = json!({ "type": "flag", "required": false });
        })
        .unwrap();
        let arguments: Vec<(String, ArgValue)> = arguments
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect();
        connector
            .operation("forecast")
            .unwrap()
            .check_arguments(&arguments, 16)
    }

    #[track_caller]
    fn assert_refused(arguments: &[(&str, ArgValue)], parts: &[&str]) {
        let problem = check(arguments).expect_err("the arguments should be refused");
        for part in parts {
            assert!(problem.contains(part), "{problem} should mention `{part}`");
        }
    }

    #[test]
    fn valid_arguments_pass() {
        let checked = check(&[
            ("city", text("paris")),
            ("days", ArgValue::Integer(7)),
            ("lang", text("pt")),
            ("metric", ArgValue::Flag(true)),
        ])
        .unwrap();
        assert_eq!(checked["city"], text("paris"));
        assert_eq!(checked["days"], ArgValue::Integer(7));
        // Optional parameters may be left out
        assert_eq!(check(&[("city", text("rome"))]).unwrap().len(), 1);
    }

    #[test]
    fn arguments_outside_their_type_are_refused_naming_the_parameter() {
        assert_refused(&[("city", text(&"a".repeat(65)))], &["city", "64"]);
        assert_refused(&[("city", text("Paris"))], &["city", "pattern"]);
        assert_refused(&[("city", text("paris!"))], &["city", "pattern"]);
        assert_refused(&[("city", text("paris")), ("lang", text("fr"))], &["lang"]);
        assert_refused(
            &[("city", text("paris")), ("days", ArgValue::Integer(0))],
            &["days", "1"],
        );
        assert_refused(
            &[("city", text("paris")), ("days", ArgValue::Integer(8))],
            &["days", "7"],
        );
        assert_refused(
            &[("city", text("paris")), ("days", text("3"))],
            &["days", "integer"],
        );
        assert_refused(&[("city", ArgValue::Flag(true))], &["city", "text"]);
        assert_refused(
            &[("city", text("paris")), ("metric", text("yes"))],
            &["metric", "flag"],
        );
        assert_refused(
            &[("city", text("paris")), ("town", text("rome"))],
            &["town", "unknown"],
        );
        assert_refused(&[("days", ArgValue::Integer(1))], &["city", "missing"]);
        assert_refused(
            &[("city", text("paris")), ("city", text("rome"))],
            &["city", "twice"],
        );
        assert_refused(&[], &["city", "missing"]);
    }

    #[test]
    fn a_path_value_is_never_empty_or_a_dot_segment() {
        let connector = parse_local(|c| {
            forecast(c)["parameters"]["city"] = json!({ "type": "text", "max_length": 8 });
        })
        .unwrap();
        let operation = connector.operation("forecast").unwrap();
        for value in ["", ".", ".."] {
            let problem = operation
                .check_arguments(&[("city".to_owned(), text(value))], 16)
                .unwrap_err();
            assert!(problem.contains("city"), "{problem}");
        }
        assert!(
            operation
                .check_arguments(&[("city".to_owned(), text("..."))], 16)
                .is_ok()
        );
    }

    #[test]
    fn too_many_arguments_are_refused() {
        let arguments: Vec<(String, ArgValue)> = (0..17)
            .map(|i| (format!("a{i}"), ArgValue::Flag(true)))
            .collect();
        let connector = parse_local(|_| {}).unwrap();
        let problem = connector
            .operation("forecast")
            .unwrap()
            .check_arguments(&arguments, 16)
            .unwrap_err();
        assert!(problem.contains("16"), "{problem}");
    }
}
