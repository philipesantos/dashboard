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

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{ErrorKind, RuntimeError};
use crate::manifest::{self, Object};

/// Largest connector file accepted, checked before parsing.
pub const MAX_CONNECTOR_BYTES: usize = 64 * 1024;

/// Where a connector comes from, which decides the rules it follows. Parsing a connector
/// adds it to nothing: the catalog is the list built into the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Built into the host from `connectors/catalog/`, reviewed line by line: it lists the
    /// domains it reserves, may fix `X-` headers, and may declare a read with a body.
    Catalog,
    /// A file of the development directory: reads are body-less `GET` or `HEAD` requests, no
    /// `X-` header, and its destinations may be this machine over plain HTTP.
    Local,
    /// A connector from a repository, whose destinations installation verifies (Phase 3). Its
    /// rules are those of a local connector without plain HTTP; nothing loads one yet.
    #[allow(dead_code)]
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
    credential: Option<Credential>,
}

/// What a connector declares about the credential its operations need
/// (`specs/005-plugin-credentials/data-model.md`, Connector: `credential`).
#[derive(Debug, Clone)]
pub struct Credential {
    kind: CredentialKind,
    required: bool,
    /// The origins of the destinations the credential is for, sorted, each once.
    origins: Vec<Origin>,
    /// `Authorization`, whatever case the connector wrote, or a catalog connector's own header.
    header: String,
    scheme: Option<String>,
    account: Option<AccountDecl>,
    /// A key's access, when declared; an account's is in [`AccountDecl`].
    access: Vec<Access>,
    subscription_required: Vec<u16>,
    digest: String,
}

/// An account the user connects, or a key the user enters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    Account,
    Key,
}

/// How an account signs in: the authorization code with PKCE (research R2).
#[derive(Debug, Clone)]
pub struct AccountDecl {
    client: String,
    authorize: url::Url,
    token: url::Url,
    revoke: Option<url::Url>,
    scopes: Vec<String>,
    access: Vec<Access>,
    identity: Option<Identity>,
    access_page: Option<url::Url>,
    revoke_page: Option<url::Url>,
}

/// The operation that tells the account's name and stable id, and where they are in its answer.
#[derive(Debug, Clone)]
pub struct Identity {
    operation: String,
    name: String,
    id: String,
}

/// One item of what the account lets the connector do, shown before approval and before sign-in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Access {
    pub name: String,
    pub level: Level,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Read,
    Write,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Read => "read",
            Level::Write => "write",
        }
    }
}

impl Credential {
    pub fn kind(&self) -> CredentialKind {
        self.kind
    }

    pub fn required(&self) -> bool {
        self.required
    }

    /// The origins the credential is sent to, sorted.
    pub fn origins(&self) -> &[Origin] {
        &self.origins
    }

    pub fn header(&self) -> &str {
        &self.header
    }

    /// `Bearer` or `token` before the secret in `Authorization`; `None` with a catalog
    /// connector's own header.
    pub fn scheme(&self) -> Option<&str> {
        self.scheme.as_deref()
    }

    /// The header's value for `secret`: `<scheme> <secret>` with `Authorization`, the secret
    /// alone with a catalog connector's own header.
    pub fn header_value(&self, secret: &str) -> String {
        match &self.scheme {
            Some(scheme) => format!("{scheme} {secret}"),
            None => secret.to_owned(),
        }
    }

    pub fn account(&self) -> Option<&AccountDecl> {
        self.account.as_ref()
    }

    /// What the credential lets the connector do: an account's access, or a key's when declared.
    pub fn access(&self) -> &[Access] {
        match &self.account {
            Some(account) => &account.access,
            None => &self.access,
        }
    }

    /// The statuses that mean a paid plan is missing.
    pub fn subscription_required(&self) -> &[u16] {
        &self.subscription_required
    }

    /// `sha256:` and the hex SHA-256 of the canonical declaration without its access, with each
    /// destination replaced by its origin (research R8): what an approval of it covers, apart
    /// from the access it compares item by item.
    pub fn digest(&self) -> String {
        self.digest.clone()
    }
}

impl AccountDecl {
    pub fn client(&self) -> &str {
        &self.client
    }

    pub fn authorize(&self) -> &url::Url {
        &self.authorize
    }

    pub fn token(&self) -> &url::Url {
        &self.token
    }

    pub fn revoke(&self) -> Option<&url::Url> {
        self.revoke.as_ref()
    }

    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }

    pub fn identity(&self) -> Option<&Identity> {
        self.identity.as_ref()
    }

    pub fn access_page(&self) -> Option<&url::Url> {
        self.access_page.as_ref()
    }

    pub fn revoke_page(&self) -> Option<&url::Url> {
        self.revoke_page.as_ref()
    }
}

impl Identity {
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// JSON pointer to the account's name in the operation's answer.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// JSON pointer to the account's stable id.
    pub fn id(&self) -> &str {
        &self.id
    }
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
    pub fn parse(bytes: &[u8], source: Source) -> Result<Connector, RuntimeError> {
        if bytes.len() > MAX_CONNECTOR_BYTES {
            return Err(invalid(format!(
                "the connector is {} bytes, over the limit of {MAX_CONNECTOR_BYTES}",
                bytes.len()
            )));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|error| invalid(format!("the connector is not UTF-8: {error}")))?;

        // First as plain JSON, refusing a key given twice at any depth (which a typed map would
        // keep the last of, unlike what a reviewer reads), and any credential field.
        let mut deserializer = serde_json::Deserializer::from_str(text);
        let Strict(document) = serde_path_to_error::deserialize(&mut deserializer)
            .map_err(|error| field_error(&error.path().to_string(), error.inner()))?;
        deserializer
            .end()
            .map_err(|error| invalid(format!("the connector is not one JSON document: {error}")))?;
        refuse_credentials(&document, "")?;

        let Object(raw): Object<RawConnector> = serde_path_to_error::deserialize(document)
            .map_err(|error| field_error(&error.path().to_string(), error.inner()))?;
        raw.check(source)
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

    /// The credential its operations need, if any.
    pub fn credential(&self) -> Option<&Credential> {
        self.credential.as_ref()
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
        let operation = self.operations.get(name)?;
        let origin = self.destinations.get(&operation.destination)?;
        let mut declaration = operation.raw.clone();
        declaration["destination"] = Value::String(origin.serialization());
        Some(sha256(canonical(&declaration).as_bytes()))
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
        if arguments.len() > max_arguments {
            return Err(format!(
                "{} arguments, more than the {max_arguments} a call may pass",
                arguments.len()
            ));
        }
        let mut checked = BTreeMap::new();
        for (name, value) in arguments {
            let Some(parameter) = self.parameters.get(name) else {
                return Err(format!("`{name}`: unknown parameter"));
            };
            if checked.contains_key(name) {
                return Err(format!("`{name}`: given twice"));
            }
            check_value(name, parameter, value, self.in_path(name))?;
            checked.insert(name.clone(), value.clone());
        }
        for (name, parameter) in &self.parameters {
            if parameter.required && !checked.contains_key(name) {
                return Err(format!("`{name}`: missing, and it is required"));
            }
        }
        Ok(checked)
    }

    /// Whether the parameter fills a path segment.
    fn in_path(&self, name: &str) -> bool {
        self.path
            .iter()
            .any(|segment| matches!(segment, Template::Parameter(parameter) if parameter == name))
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

/// One argument's value against its parameter's type.
fn check_value(
    name: &str,
    parameter: &Parameter,
    value: &ArgValue,
    in_path: bool,
) -> Result<(), String> {
    match (&parameter.kind, value) {
        (
            ParameterType::Text {
                max_length,
                pattern,
                values,
            },
            ArgValue::Text(text),
        ) => {
            let length = text.chars().count();
            if length > *max_length {
                return Err(format!(
                    "`{name}`: longer than {max_length} characters ({length})"
                ));
            }
            if let Some((_, pattern)) = pattern
                && !pattern.is_match(text)
            {
                return Err(format!("`{name}`: does not match its pattern"));
            }
            if let Some(values) = values
                && !values.contains(text)
            {
                return Err(format!("`{name}`: not one of the allowed values"));
            }
            if in_path && matches!(text.as_str(), "" | "." | "..") {
                return Err(format!(
                    "`{name}`: fills a path segment, which cannot be empty, `.` or `..`"
                ));
            }
            Ok(())
        }
        (ParameterType::Integer { minimum, maximum }, ArgValue::Integer(number)) => {
            if (*minimum..=*maximum).contains(number) {
                Ok(())
            } else {
                Err(format!(
                    "`{name}`: {number} is outside {minimum} to {maximum}"
                ))
            }
        }
        (ParameterType::Flag, ArgValue::Flag(_)) => Ok(()),
        (ParameterType::Text { .. }, _) => Err(format!("`{name}`: must be text")),
        (ParameterType::Integer { .. }, _) => Err(format!("`{name}`: must be an integer")),
        (ParameterType::Flag, _) => Err(format!("`{name}`: must be a flag, true or false")),
    }
}

/// The limits of `wit/connector.schema.json`, in characters (Unicode scalar values), as JSON
/// Schema counts them, or in entries.
const MAX_SCHEMA_CHARS: usize = 2048;
const MAX_ID_CHARS: usize = 160;
const MAX_VERSION_CHARS: usize = 64;
const MAX_AUTHOR_CHARS: usize = 100;
const MAX_DESCRIPTION_CHARS: usize = 200;
const MAX_RESERVES: usize = 8;
const MAX_DOMAIN_CHARS: usize = 253;
const MAX_DESTINATIONS: usize = 8;
const MAX_ORIGIN_CHARS: usize = 260;
const MAX_OPERATIONS: usize = 64;
const MAX_PATH_CHARS: usize = 512;
const MAX_QUERY_PAIRS: usize = 16;
const MAX_QUERY_NAME_CHARS: usize = 64;
const MAX_FIXED_VALUE_CHARS: usize = 256;
const MAX_HEADERS: usize = 8;
const MAX_HEADER_SUFFIX_CHARS: usize = 60;
const MAX_BODY_BYTES: usize = 4 * 1024;
const MAX_PARAMETERS: usize = 16;
const MAX_TEXT_LENGTH: u64 = 2048;
const MAX_PATTERN_CHARS: usize = 512;
const MAX_PATTERN_COMPILED_BYTES: usize = 64 * 1024;
const MAX_VALUES: usize = 64;
const MAX_VALUE_CHARS: usize = 2048;
const MAX_RESPONSE_HEADERS: usize = 8;
const MAX_RESPONSE_HEADER_CHARS: usize = 64;
/// `[a-z][a-z0-9-]{0,31}` and `[a-z][a-z0-9-]{0,63}`.
const MAX_DESTINATION_NAME_CHARS: usize = 32;
const MAX_OPERATION_NAME_CHARS: usize = 64;
/// `[a-z][a-z0-9_]{0,31}`
const MAX_PARAMETER_NAME_CHARS: usize = 32;

/// Field names that declare a credential, refused anywhere but the top-level `credential`
/// (feature 003's FR-017, feature 005's research R3).
const CREDENTIAL_FIELDS: &[&str] = &["credential", "credentials", "auth"];

const CATALOG_PREFIX: &str = "catalog/";
const LOCAL_PREFIX: &str = "local/";

/// The connector as written, before the checks that types cannot express.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConnector {
    /// Lets editors find the schema; ignored. A string when present: `null` is not accepted.
    #[serde(rename = "$schema", default, deserialize_with = "present")]
    schema: Option<String>,
    /// Any JSON number: the schema compares it with 1 by value, so `1.0` is 1.
    connector_version: serde_json::Number,
    id: String,
    version: String,
    author: String,
    description: String,
    #[serde(default, deserialize_with = "present")]
    reserves: Option<Vec<String>>,
    destinations: BTreeMap<String, String>,
    /// Read as JSON first, so each operation keeps its declaration as written for its digest.
    operations: BTreeMap<String, Value>,
    /// Read as JSON first, so the credential keeps its declaration as written for its digest.
    #[serde(default, deserialize_with = "present")]
    credential: Option<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOperation {
    kind: RawKind,
    method: RawMethod,
    destination: String,
    path: String,
    #[serde(default, deserialize_with = "present")]
    query: Option<BTreeMap<String, String>>,
    #[serde(default, deserialize_with = "present")]
    headers: Option<BTreeMap<String, String>>,
    #[serde(default, deserialize_with = "present")]
    body: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    parameters: Option<BTreeMap<String, Object<RawParameter>>>,
    response: Object<RawResponse>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum RawKind {
    Read,
    Change,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "UPPERCASE")]
enum RawMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

/// Every field any type may have; which ones a type allows is checked after.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawParameter {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "present")]
    required: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    max_length: Option<u64>,
    #[serde(default, deserialize_with = "present")]
    pattern: Option<String>,
    #[serde(default, deserialize_with = "present")]
    values: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    minimum: Option<i64>,
    #[serde(default, deserialize_with = "present")]
    maximum: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResponse {
    format: RawFormat,
    #[serde(default, deserialize_with = "present")]
    headers: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum RawFormat {
    Json,
    Text,
}

/// An optional field that, when present, is never `null`.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

impl RawConnector {
    fn check(self, source: Source) -> Result<Connector, RuntimeError> {
        if let Some(schema) = &self.schema
            && schema.chars().count() > MAX_SCHEMA_CHARS
        {
            return Err(field(
                "$schema",
                format!("longer than {MAX_SCHEMA_CHARS} characters"),
            ));
        }
        if self.connector_version.as_f64() != Some(1.0) {
            return Err(field(
                "connector_version",
                format!("must be 1, found {}", self.connector_version),
            ));
        }
        check_id(&self.id, source)?;
        let version = check_version(&self.version)?;
        check_shown_text("author", &self.author, MAX_AUTHOR_CHARS)?;
        check_shown_text("description", &self.description, MAX_DESCRIPTION_CHARS)?;
        let reserves = check_reserves(self.reserves, source)?;

        if !(1..=MAX_DESTINATIONS).contains(&self.destinations.len()) {
            return Err(field(
                "destinations",
                format!("must have 1 to {MAX_DESTINATIONS} entries"),
            ));
        }
        let mut destinations = BTreeMap::new();
        for (name, origin) in &self.destinations {
            if !is_name(name, MAX_DESTINATION_NAME_CHARS, '-') {
                return Err(field(
                    "destinations",
                    format!(
                        "`{name}` must be a lowercase letter, then up to 31 lowercase letters, digits or hyphens"
                    ),
                ));
            }
            let origin = Origin::parse(origin, source)
                .map_err(|problem| field(&format!("destinations.{name}"), problem))?;
            destinations.insert(name.clone(), origin);
        }

        if source == Source::Catalog {
            for origin in destinations.values() {
                if !reserves.iter().any(|domain| is_under(&origin.host, domain)) {
                    return Err(field(
                        "reserves",
                        format!(
                            "no reserved domain covers {}, a host the connector declares",
                            origin.host
                        ),
                    ));
                }
            }
        }

        if !(1..=MAX_OPERATIONS).contains(&self.operations.len()) {
            return Err(field(
                "operations",
                format!("must have 1 to {MAX_OPERATIONS} entries"),
            ));
        }
        let mut operations = BTreeMap::new();
        for (name, declaration) in self.operations {
            if !is_name(&name, MAX_OPERATION_NAME_CHARS, '-') {
                return Err(field(
                    "operations",
                    format!(
                        "`{name}` must be a lowercase letter, then up to 63 lowercase letters, digits or hyphens"
                    ),
                ));
            }
            let Object(raw): Object<RawOperation> =
                serde_path_to_error::deserialize(declaration.clone()).map_err(|error| {
                    let path = error.path().to_string();
                    let at = if path == "." {
                        format!("operations.{name}")
                    } else {
                        format!("operations.{name}.{path}")
                    };
                    field(&at, error.inner())
                })?;
            let mut operation = raw.check(&name, &destinations, source)?;
            operation.raw = declaration;
            operations.insert(name, operation);
        }

        let credential = match self.credential {
            Some(declaration) => Some(check_credential(
                declaration,
                &destinations,
                &operations,
                &reserves,
                source,
            )?),
            None => None,
        };

        Ok(Connector {
            id: self.id,
            version,
            author: self.author,
            description: self.description,
            reserves,
            destinations,
            operations,
            credential,
        })
    }
}

impl RawOperation {
    fn check(
        self,
        name: &str,
        destinations: &BTreeMap<String, Origin>,
        source: Source,
    ) -> Result<Operation, RuntimeError> {
        let at = |field_name: &str| format!("operations.{name}.{field_name}");
        let kind = match self.kind {
            RawKind::Read => Kind::Read,
            RawKind::Change => Kind::Change,
        };
        let method = match self.method {
            RawMethod::Get => Method::Get,
            RawMethod::Head => Method::Head,
            RawMethod::Post => Method::Post,
            RawMethod::Put => Method::Put,
            RawMethod::Patch => Method::Patch,
            RawMethod::Delete => Method::Delete,
        };
        // Outside the catalog the host can check a read only by its shape (research R4)
        if source != Source::Catalog
            && kind == Kind::Read
            && (!matches!(method, Method::Get | Method::Head) || self.body.is_some())
        {
            return Err(field(
                &format!("operations.{name}"),
                "outside the catalog, a read is a GET or HEAD request without a body; declare this operation as a change",
            ));
        }
        if !destinations.contains_key(&self.destination) {
            return Err(field(
                &at("destination"),
                format!("`{}` is not a key of destinations", self.destination),
            ));
        }
        let path = check_path(&self.path).map_err(|problem| field(&at("path"), problem))?;
        let query = check_query(self.query.unwrap_or_default(), &at("query"))?;
        let headers = check_headers(self.headers.unwrap_or_default(), &at("headers"), source)?;
        let body = match self.body {
            None => None,
            Some(body) => Some(check_body(body).map_err(|problem| field(&at("body"), problem))?),
        };
        let parameters = check_parameters(self.parameters.unwrap_or_default(), &at("parameters"))?;
        let Object(response) = self.response;
        let response = check_response(response, &at("response"))?;

        let operation = Operation {
            kind,
            method,
            destination: self.destination,
            path,
            query,
            headers,
            body,
            parameters,
            response,
            raw: Value::Null,
        };
        operation
            .check_placeholders()
            .map_err(|problem| field(&format!("operations.{name}"), problem))?;
        Ok(operation)
    }
}

impl Operation {
    /// Every placeholder names a declared parameter, every parameter appears in exactly one
    /// place, and an optional one never fills a path segment.
    fn check_placeholders(&self) -> Result<(), String> {
        let mut uses: Vec<&str> = Vec::new();
        for segment in &self.path {
            if let Template::Parameter(name) = segment {
                if self
                    .parameters
                    .get(name)
                    .is_some_and(|parameter| !parameter.required)
                {
                    return Err(format!(
                        "`{name}` is optional, and an optional parameter cannot fill a path segment"
                    ));
                }
                uses.push(name);
            }
        }
        for (_, value) in &self.query {
            if let Template::Parameter(name) = value {
                uses.push(name);
            }
        }
        if let Some(body) = &self.body {
            body_placeholders(body, &mut uses);
        }
        for name in &uses {
            if !self.parameters.contains_key(*name) {
                return Err(format!("`{{{name}}}` names no declared parameter"));
            }
        }
        for name in self.parameters.keys() {
            match uses.iter().filter(|used| **used == name.as_str()).count() {
                0 => return Err(format!("parameter `{name}` appears nowhere in the request")),
                1 => {}
                _ => return Err(format!("parameter `{name}` appears in more than one place")),
            }
        }
        Ok(())
    }
}

fn body_placeholders<'a>(value: &'a Value, uses: &mut Vec<&'a str>) {
    match value {
        Value::String(text) => {
            if let Some(name) = placeholder(text) {
                uses.push(name);
            }
        }
        Value::Array(items) => {
            for item in items {
                body_placeholders(item, uses);
            }
        }
        Value::Object(fields) => {
            for item in fields.values() {
                body_placeholders(item, uses);
            }
        }
        _ => {}
    }
}

impl Origin {
    /// `https://<domain>[:<port>]`, or for a local connector `http://localhost:<port>`,
    /// `http://127.0.0.1:<port>` or `http://[::1]:<port>`, with nothing after the port but an
    /// optional `/`.
    fn parse(raw: &str, source: Source) -> Result<Origin, String> {
        if raw.chars().count() > MAX_ORIGIN_CHARS {
            return Err(format!("longer than {MAX_ORIGIN_CHARS} characters"));
        }
        let rest = raw.strip_suffix('/').unwrap_or(raw);
        let https = if let Some(authority) = rest.strip_prefix("https://") {
            if authority.is_empty()
                || authority.chars().any(|c| {
                    matches!(c, '/' | '?' | '#' | '@') || c.is_whitespace() || c.is_control()
                })
            {
                return Err(format!(
                    "`{raw}` must be https://<domain>[:<port>], with no path, query, fragment, user or password"
                ));
            }
            true
        } else if let Some(authority) = rest.strip_prefix("http://") {
            let loopback = ["localhost:", "127.0.0.1:", "[::1]:"].iter().any(|host| {
                authority.strip_prefix(host).is_some_and(|port| {
                    (1..=5).contains(&port.len()) && port.bytes().all(|b| b.is_ascii_digit())
                })
            });
            if !loopback {
                return Err(format!(
                    "`{raw}` uses plain HTTP, allowed only for a local connector on this machine: http://localhost:<port>, http://127.0.0.1:<port> or http://[::1]:<port>"
                ));
            }
            if source != Source::Local {
                return Err(format!(
                    "`{raw}` uses plain HTTP, which only a local connector may, in development"
                ));
            }
            false
        } else {
            return Err(format!("`{raw}` must start with https://"));
        };

        let url = url::Url::parse(raw).map_err(|error| format!("`{raw}` is not a URL: {error}"))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| format!("`{raw}` has no port"))?;
        if port == 0 {
            return Err(format!("`{raw}` has port 0"));
        }
        let host = match url.host() {
            Some(url::Host::Domain(domain)) if https => {
                if domain.ends_with('.') {
                    return Err(format!(
                        "`{raw}` ends with a dot; write the host without it"
                    ));
                }
                domain.to_owned()
            }
            Some(url::Host::Domain(domain)) if domain == "localhost" => domain.to_owned(),
            Some(url::Host::Ipv4(address)) if !https && address.is_loopback() => {
                address.to_string()
            }
            Some(url::Host::Ipv6(address)) if !https && address.is_loopback() => {
                format!("[{address}]")
            }
            Some(url::Host::Ipv4(_) | url::Host::Ipv6(_)) => {
                return Err(format!(
                    "`{raw}` is an IP address; a destination is a DNS name"
                ));
            }
            _ => return Err(format!("`{raw}` has no host")),
        };
        Ok(Origin { https, host, port })
    }
}

/// Whether `host` is `domain` or a host under it.
pub(crate) fn is_under(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn check_id(id: &str, source: Source) -> Result<(), RuntimeError> {
    if id.chars().count() > MAX_ID_CHARS {
        return Err(field(
            "id",
            format!("longer than {MAX_ID_CHARS} characters"),
        ));
    }
    let catalog = id
        .strip_prefix(CATALOG_PREFIX)
        .is_some_and(manifest::is_local_name);
    let local = id
        .strip_prefix(LOCAL_PREFIX)
        .is_some_and(manifest::is_local_name);
    let repository = manifest::is_repository_id(id);
    match source {
        Source::Catalog if catalog => Ok(()),
        Source::Catalog => Err(field(
            "id",
            format!("`{id}`: a catalog connector's id is {CATALOG_PREFIX}<name>, in lowercase"),
        )),
        _ if catalog => Err(RuntimeError::new(
            ErrorKind::DestinationReserved,
            format!(
                "id: `{id}` is a catalog id, which only the catalog built into the host may use"
            ),
        )),
        Source::Local if local => Ok(()),
        Source::Local => Err(field(
            "id",
            format!("`{id}`: a local connector's id is {LOCAL_PREFIX}<name>, in lowercase"),
        )),
        Source::Repository if repository => Ok(()),
        Source::Repository => Err(field(
            "id",
            format!(
                "`{id}`: a repository connector's id is github.com/<owner>/<repository>, in lowercase"
            ),
        )),
    }
}

fn check_version(version: &str) -> Result<semver::Version, RuntimeError> {
    if version.chars().count() > MAX_VERSION_CHARS {
        return Err(field(
            "version",
            format!("longer than {MAX_VERSION_CHARS} characters"),
        ));
    }
    semver::Version::parse(version).map_err(|error| {
        field(
            "version",
            format!("`{version}` is not SemVer 2.0.0: {error}"),
        )
    })
}

/// 1 to `max` characters, none of them a control character, as the schema's `shown_text`.
fn check_shown_text(name: &str, text: &str, max: usize) -> Result<(), RuntimeError> {
    let length = text.chars().count();
    if !(1..=max).contains(&length) {
        return Err(field(
            name,
            format!("must have 1 to {max} characters, found {length}"),
        ));
    }
    if text.chars().any(char::is_control) {
        return Err(field(name, "contains a control character"));
    }
    Ok(())
}

fn check_reserves(
    reserves: Option<Vec<String>>,
    source: Source,
) -> Result<Vec<String>, RuntimeError> {
    match (source, reserves) {
        (Source::Catalog, None) => Err(field(
            "reserves",
            "a catalog connector lists the domains it reserves",
        )),
        (Source::Catalog, Some(reserves)) => {
            if !(1..=MAX_RESERVES).contains(&reserves.len()) {
                return Err(field(
                    "reserves",
                    format!("must have 1 to {MAX_RESERVES} entries"),
                ));
            }
            for (index, domain) in reserves.iter().enumerate() {
                if !is_domain(domain) {
                    return Err(field(
                        &format!("reserves[{index}]"),
                        format!("`{domain}` is not a domain in lowercase, such as github.com"),
                    ));
                }
                if reserves[..index].contains(domain) {
                    return Err(field("reserves", format!("`{domain}` is listed twice")));
                }
            }
            Ok(reserves)
        }
        (_, Some(_)) => Err(field("reserves", "only catalog connectors reserve domains")),
        (_, None) => Ok(Vec::new()),
    }
}

/// `(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{0,61}[a-z0-9]`, at most 253
/// characters: two labels or more, the last starting with a letter.
fn is_domain(domain: &str) -> bool {
    let labels: Vec<&str> = domain.split('.').collect();
    let label = |label: &str| {
        let alphanumeric = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
        (1..=63).contains(&label.len())
            && label.chars().all(|c| alphanumeric(c) || c == '-')
            && label.starts_with(alphanumeric)
            && label.ends_with(alphanumeric)
    };
    domain.len() <= MAX_DOMAIN_CHARS
        && labels.len() >= 2
        && labels.iter().all(|part| label(part))
        && labels.last().is_some_and(|last| {
            last.len() >= 2 && last.starts_with(|c: char| c.is_ascii_lowercase())
        })
}

/// `[a-z]` then up to `max - 1` of `[a-z0-9]` and `extra`.
fn is_name(name: &str, max: usize, extra: char) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && name.len() <= max
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == extra)
}

/// The name in a placeholder, which is exactly `{<parameter>}`.
pub(crate) fn placeholder_name(text: &str) -> Option<&str> {
    placeholder(text)
}

/// Whether `text` is exactly `{<parameter>}`, a value the widget fills in rather than a fixed
/// one.
pub fn is_placeholder(text: &str) -> bool {
    placeholder(text).is_some()
}

fn placeholder(text: &str) -> Option<&str> {
    let name = text.strip_prefix('{')?.strip_suffix('}')?;
    is_name(name, MAX_PARAMETER_NAME_CHARS, '_').then_some(name)
}

fn template(text: &str) -> Template {
    match placeholder(text) {
        Some(name) => Template::Parameter(name.to_owned()),
        None => Template::Literal(text.to_owned()),
    }
}

/// RFC 3986 `pchar`s, without `{` and `}`.
fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "._~!$&'()*+,;=:@%-".contains(c)
}

fn check_path(path: &str) -> Result<Vec<Template>, String> {
    if path.chars().count() > MAX_PATH_CHARS {
        return Err(format!("longer than {MAX_PATH_CHARS} characters"));
    }
    let Some(rest) = path.strip_prefix('/') else {
        return Err(format!("`{path}` must start with /"));
    };
    rest.split('/')
        .map(|segment| match placeholder(segment) {
            Some(name) => Ok(Template::Parameter(name.to_owned())),
            None if segment.chars().all(is_path_char) => Ok(Template::Literal(segment.to_owned())),
            None => Err(format!(
                "`{segment}` is neither a literal segment (letters, digits and . _ ~ ! $ & ' ( ) * + , ; = : @ % -) nor exactly {{<parameter>}}"
            )),
        })
        .collect()
}

fn check_query(
    query: BTreeMap<String, String>,
    at: &str,
) -> Result<Vec<(String, Template)>, RuntimeError> {
    if query.len() > MAX_QUERY_PAIRS {
        return Err(field(at, format!("more than {MAX_QUERY_PAIRS} pairs")));
    }
    let mut pairs = Vec::new();
    for (name, value) in query {
        let valid_name = (1..=MAX_QUERY_NAME_CHARS).contains(&name.chars().count())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._~-".contains(c));
        if !valid_name {
            return Err(field(
                at,
                format!(
                    "`{name}` must have 1 to {MAX_QUERY_NAME_CHARS} letters, digits or . _ ~ -"
                ),
            ));
        }
        if value.chars().count() > MAX_FIXED_VALUE_CHARS {
            return Err(field(
                &format!("{at}.{name}"),
                format!("longer than {MAX_FIXED_VALUE_CHARS} characters"),
            ));
        }
        let value = template(&value);
        pairs.push((name, value));
    }
    Ok(pairs)
}

fn check_headers(
    headers: BTreeMap<String, String>,
    at: &str,
    source: Source,
) -> Result<Vec<(String, String)>, RuntimeError> {
    if headers.len() > MAX_HEADERS {
        return Err(field(at, format!("more than {MAX_HEADERS} headers")));
    }
    for (name, value) in &headers {
        let x_header = name.strip_prefix("X-").is_some_and(|suffix| {
            (1..=MAX_HEADER_SUFFIX_CHARS).contains(&suffix.len())
                && suffix
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
        if x_header && source != Source::Catalog {
            return Err(field(
                at,
                format!(
                    "`{name}`: only catalog connectors fix X- headers, since some servers read them to change a request's method or path"
                ),
            ));
        }
        if !x_header && name != "Accept" && name != "Accept-Language" {
            return Err(field(
                at,
                format!(
                    "`{name}`: a connector fixes only Accept, Accept-Language and, in the catalog, X- headers; the host sets or forbids the others"
                ),
            ));
        }
        if value.chars().count() > MAX_FIXED_VALUE_CHARS {
            return Err(field(
                &format!("{at}.{name}"),
                format!("longer than {MAX_FIXED_VALUE_CHARS} characters"),
            ));
        }
        if !value.chars().all(|c| (' '..='~').contains(&c)) {
            return Err(field(
                &format!("{at}.{name}"),
                "only printable ASCII characters",
            ));
        }
    }
    Ok(headers.into_iter().collect())
}

fn check_body(body: Value) -> Result<Value, String> {
    if !body.is_object() {
        return Err("must be a JSON object".to_owned());
    }
    let written = body.to_string().len();
    if written > MAX_BODY_BYTES {
        return Err(format!(
            "{written} bytes as written, over the limit of {MAX_BODY_BYTES}"
        ));
    }
    Ok(body)
}

fn check_parameters(
    parameters: BTreeMap<String, Object<RawParameter>>,
    at: &str,
) -> Result<BTreeMap<String, Parameter>, RuntimeError> {
    if parameters.len() > MAX_PARAMETERS {
        return Err(field(at, format!("more than {MAX_PARAMETERS} parameters")));
    }
    let mut checked = BTreeMap::new();
    for (name, Object(raw)) in parameters {
        if !is_name(&name, MAX_PARAMETER_NAME_CHARS, '_') {
            return Err(field(
                at,
                format!(
                    "`{name}` must be a lowercase letter, then up to 31 lowercase letters, digits or underscores"
                ),
            ));
        }
        let parameter = raw
            .check()
            .map_err(|problem| field(&format!("{at}.{name}"), problem))?;
        checked.insert(name, parameter);
    }
    Ok(checked)
}

impl RawParameter {
    fn check(self) -> Result<Parameter, String> {
        let required = self.required.unwrap_or(true);
        let only = |allowed: &[&str]| -> Result<(), String> {
            let given = [
                ("max_length", self.max_length.is_some()),
                ("pattern", self.pattern.is_some()),
                ("values", self.values.is_some()),
                ("minimum", self.minimum.is_some()),
                ("maximum", self.maximum.is_some()),
            ];
            match given
                .iter()
                .find(|(name, present)| *present && !allowed.contains(name))
            {
                Some((name, _)) => Err(format!("a {} parameter has no `{name}`", self.kind)),
                None => Ok(()),
            }
        };
        let kind = match self.kind.as_str() {
            "text" => {
                only(&["max_length", "pattern", "values"])?;
                let max_length = self
                    .max_length
                    .ok_or("a text parameter needs `max_length`")?;
                if !(1..=MAX_TEXT_LENGTH).contains(&max_length) {
                    return Err(format!("`max_length` must be 1 to {MAX_TEXT_LENGTH}"));
                }
                if self.pattern.is_some() && self.values.is_some() {
                    return Err("a text parameter has `pattern` or `values`, not both".to_owned());
                }
                let pattern = match self.pattern {
                    None => None,
                    Some(pattern) => Some(compile_pattern(pattern)?),
                };
                let values = match self.values {
                    None => None,
                    Some(values) => Some(check_values(values)?),
                };
                ParameterType::Text {
                    max_length: max_length as usize,
                    pattern,
                    values,
                }
            }
            "integer" => {
                only(&["minimum", "maximum"])?;
                let (Some(minimum), Some(maximum)) = (self.minimum, self.maximum) else {
                    return Err("an integer parameter needs `minimum` and `maximum`".to_owned());
                };
                if minimum > maximum {
                    return Err(format!(
                        "`minimum` {minimum} is greater than `maximum` {maximum}"
                    ));
                }
                ParameterType::Integer { minimum, maximum }
            }
            "flag" => {
                only(&[])?;
                ParameterType::Flag
            }
            other => {
                return Err(format!(
                    "`type` is `{other}`; it must be text, integer or flag"
                ));
            }
        };
        Ok(Parameter { kind, required })
    }
}

/// A pattern matches a whole value; the `regex` crate runs in linear time, and the compiled
/// pattern is capped (research R3).
fn compile_pattern(pattern: String) -> Result<(String, regex::Regex), String> {
    let length = pattern.chars().count();
    if !(1..=MAX_PATTERN_CHARS).contains(&length) {
        return Err(format!(
            "`pattern` must have 1 to {MAX_PATTERN_CHARS} characters"
        ));
    }
    let compiled = regex::RegexBuilder::new(&format!("^(?:{pattern})$"))
        .size_limit(MAX_PATTERN_COMPILED_BYTES)
        .dfa_size_limit(MAX_PATTERN_COMPILED_BYTES)
        .build()
        .map_err(|error| format!("`pattern` is not a valid pattern: {error}"))?;
    Ok((pattern, compiled))
}

fn check_values(values: Vec<String>) -> Result<Vec<String>, String> {
    if !(1..=MAX_VALUES).contains(&values.len()) {
        return Err(format!("`values` must have 1 to {MAX_VALUES} entries"));
    }
    for (index, value) in values.iter().enumerate() {
        if value.chars().count() > MAX_VALUE_CHARS {
            return Err(format!(
                "`values[{index}]` is longer than {MAX_VALUE_CHARS} characters"
            ));
        }
        if values[..index].contains(value) {
            return Err(format!("`values` lists `{value}` twice"));
        }
    }
    Ok(values)
}

fn check_response(response: RawResponse, at: &str) -> Result<ResponseSpec, RuntimeError> {
    let format = match response.format {
        RawFormat::Json => Format::Json,
        RawFormat::Text => Format::Text,
    };
    let headers = response.headers.unwrap_or_default();
    let at = format!("{at}.headers");
    if headers.len() > MAX_RESPONSE_HEADERS {
        return Err(field(
            &at,
            format!("more than {MAX_RESPONSE_HEADERS} names"),
        ));
    }
    for (index, name) in headers.iter().enumerate() {
        let valid = (1..=MAX_RESPONSE_HEADER_CHARS).contains(&name.len())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !valid {
            return Err(field(
                &at,
                format!(
                    "`{name}` must have 1 to {MAX_RESPONSE_HEADER_CHARS} letters, digits or hyphens"
                ),
            ));
        }
        if name.eq_ignore_ascii_case("set-cookie") || name.eq_ignore_ascii_case("cookie") {
            return Err(field(
                &at,
                format!("`{name}`: cookies never reach a widget"),
            ));
        }
        if headers[..index].contains(name) {
            return Err(field(&at, format!("`{name}` is listed twice")));
        }
    }
    Ok(ResponseSpec { format, headers })
}

/// The credential as written (`data-model.md`, Connector: `credential`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCredential {
    kind: RawCredentialKind,
    #[serde(default)]
    required: bool,
    destinations: Vec<String>,
    header: String,
    #[serde(default, deserialize_with = "present")]
    scheme: Option<String>,
    #[serde(default, deserialize_with = "present")]
    account: Option<Object<RawAccount>>,
    #[serde(default, deserialize_with = "present")]
    access: Option<Vec<Object<Access>>>,
    #[serde(default, deserialize_with = "present")]
    subscription_required: Option<Vec<u64>>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum RawCredentialKind {
    Account,
    Key,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAccount {
    flow: RawFlow,
    client: String,
    authorize: String,
    token: String,
    #[serde(default, deserialize_with = "present")]
    revoke: Option<String>,
    #[serde(default, deserialize_with = "present")]
    scopes: Option<Vec<String>>,
    access: Vec<Object<Access>>,
    #[serde(default, deserialize_with = "present")]
    identity: Option<Object<RawIdentity>>,
    #[serde(default, deserialize_with = "present")]
    access_page: Option<String>,
    #[serde(default, deserialize_with = "present")]
    revoke_page: Option<String>,
}

#[derive(Deserialize, Clone, Copy)]
enum RawFlow {
    #[serde(rename = "authorization-code")]
    AuthorizationCode,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIdentity {
    operation: String,
    name: String,
    id: String,
}

const MAX_CREDENTIAL_DESTINATIONS: usize = 8;
const MAX_HEADER_NAME_CHARS: usize = 64;
const MAX_SCHEME_CHARS: usize = 32;
const MAX_SCOPES: usize = 32;
const MAX_SCOPE_CHARS: usize = 64;
const MAX_ACCESS: usize = 32;
const MAX_ACCESS_NAME_CHARS: usize = 64;
const MAX_SUBSCRIPTION_STATUSES: usize = 4;
const MAX_CLIENT_CHARS: usize = 32;
const MAX_POINTER_SEGMENTS: usize = 8;
const MAX_POINTER_SEGMENT_CHARS: usize = 64;
const MAX_ADDRESS_CHARS: usize = 2048;
/// Headers the host sets or forbids itself: no credential travels in one (feature 003, R1).
const HOST_HEADERS: &[&str] = &[
    "accept",
    "accept-language",
    "accept-encoding",
    "connection",
    "content-length",
    "content-type",
    "cookie",
    "host",
    "te",
    "transfer-encoding",
    "upgrade",
    "user-agent",
];
const AUTHORIZATION: &str = "Authorization";

/// Checks a connector's `credential` in the order of `data-model.md` (Connector: `credential`).
fn check_credential(
    declaration: Value,
    destinations: &BTreeMap<String, Origin>,
    operations: &BTreeMap<String, Operation>,
    reserves: &[String],
    source: Source,
) -> Result<Credential, RuntimeError> {
    let at = |path: &str| {
        if path == "." {
            "credential".to_owned()
        } else {
            format!("credential.{path}")
        }
    };
    // 1. The schema
    let Object(raw): Object<RawCredential> = serde_path_to_error::deserialize(declaration.clone())
        .map_err(|error| field(&at(&error.path().to_string()), error.inner()))?;
    let kind = match raw.kind {
        RawCredentialKind::Account => CredentialKind::Account,
        RawCredentialKind::Key => CredentialKind::Key,
    };
    match (kind, &raw.account, &raw.access) {
        (CredentialKind::Account, None, _) => {
            return Err(field(
                "credential.account",
                "an account declares how it signs in",
            ));
        }
        (CredentialKind::Account, Some(_), Some(_)) => {
            return Err(field(
                "credential.access",
                "an account declares its access in credential.account.access",
            ));
        }
        (CredentialKind::Key, Some(_), _) => {
            return Err(field(
                "credential.account",
                "only an account signs in; a key does not",
            ));
        }
        _ => {}
    }
    let subscription_required = check_statuses(raw.subscription_required.unwrap_or_default())?;
    let key_access = match raw.access {
        Some(access) => check_access(access, "credential.access")?,
        None => Vec::new(),
    };

    // 2. The destinations it is for, and HTTPS for every destination of the connector
    if !(1..=MAX_CREDENTIAL_DESTINATIONS).contains(&raw.destinations.len()) {
        return Err(field(
            "credential.destinations",
            format!("must have 1 to {MAX_CREDENTIAL_DESTINATIONS} entries"),
        ));
    }
    let mut origins = Vec::new();
    for (index, name) in raw.destinations.iter().enumerate() {
        if raw.destinations[..index].contains(name) {
            return Err(field(
                "credential.destinations",
                format!("`{name}` is listed twice"),
            ));
        }
        let origin = destinations.get(name).ok_or_else(|| {
            field(
                "credential.destinations",
                format!("`{name}` is not one of the connector's destinations"),
            )
        })?;
        origins.push(origin.clone());
    }
    origins.sort();
    origins.dedup();
    if let Some((name, origin)) = destinations.iter().find(|(_, origin)| !origin.is_https()) {
        return Err(RuntimeError::new(
            ErrorKind::CredentialNeedsHttps,
            format!(
                "destinations.{name}: {origin} is not HTTPS; a connector that declares a credential reaches every destination over HTTPS, since only one that sends no credential may go without it"
            ),
        ));
    }

    // 3. The header, which the host alone writes
    let header = check_credential_header(&raw.header, raw.scheme.as_deref(), operations, source)?;
    let scheme = if header == AUTHORIZATION {
        raw.scheme
    } else {
        None
    };

    // 4–6. The account: addresses, identity, client
    let account = match raw.account {
        None => None,
        Some(Object(account)) => Some(check_account(
            account,
            destinations,
            &origins,
            &raw.destinations,
            operations,
            reserves,
            source,
        )?),
    };

    let mut digested = declaration;
    if let Value::Object(fields) = &mut digested {
        fields.remove("access");
        if let Some(Value::Object(account)) = fields.get_mut("account") {
            account.remove("access");
        }
        fields.insert(
            "destinations".to_owned(),
            Value::Array(
                origins
                    .iter()
                    .map(|origin| Value::String(origin.serialization()))
                    .collect(),
            ),
        );
    }
    Ok(Credential {
        kind,
        required: raw.required,
        origins,
        header,
        scheme,
        account,
        access: key_access,
        subscription_required,
        digest: sha256(canonical(&digested).as_bytes()),
    })
}

fn check_statuses(statuses: Vec<u64>) -> Result<Vec<u16>, RuntimeError> {
    let at = "credential.subscription_required";
    if statuses.len() > MAX_SUBSCRIPTION_STATUSES {
        return Err(field(
            at,
            format!("at most {MAX_SUBSCRIPTION_STATUSES} statuses"),
        ));
    }
    let mut kept: Vec<u16> = Vec::new();
    for status in statuses {
        let status = u16::try_from(status)
            .ok()
            .filter(|status| (400..=499).contains(status))
            .ok_or_else(|| field(at, format!("{status} is not a status from 400 to 499")))?;
        if kept.contains(&status) {
            return Err(field(at, format!("{status} is listed twice")));
        }
        kept.push(status);
    }
    Ok(kept)
}

fn check_access(access: Vec<Object<Access>>, at: &str) -> Result<Vec<Access>, RuntimeError> {
    if !(1..=MAX_ACCESS).contains(&access.len()) {
        return Err(field(at, format!("must have 1 to {MAX_ACCESS} items")));
    }
    let mut items: Vec<Access> = Vec::new();
    for (index, Object(item)) in access.into_iter().enumerate() {
        check_shown_text(
            &format!("{at}[{index}].name"),
            &item.name,
            MAX_ACCESS_NAME_CHARS,
        )?;
        if items.iter().any(|kept| kept.name == item.name) {
            return Err(field(at, format!("`{}` is listed twice", item.name)));
        }
        items.push(item);
    }
    items.sort();
    Ok(items)
}

/// `Authorization` with a scheme, or in the catalog another header; never one the host sets or
/// forbids, and never one an operation fixes, compared without regard to case.
fn check_credential_header(
    header: &str,
    scheme: Option<&str>,
    operations: &BTreeMap<String, Operation>,
    source: Source,
) -> Result<String, RuntimeError> {
    let well_formed = header
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && header.chars().count() <= MAX_HEADER_NAME_CHARS
        && header
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !well_formed {
        return Err(field(
            "credential.header",
            format!("`{header}` must be a letter, then up to 63 letters, digits or hyphens"),
        ));
    }
    let lower = header.to_ascii_lowercase();
    let header = if header.eq_ignore_ascii_case(AUTHORIZATION) {
        let scheme = scheme.ok_or_else(|| {
            field(
                "credential.scheme",
                "a credential in Authorization names its scheme, such as Bearer",
            )
        })?;
        let token = (1..=MAX_SCHEME_CHARS).contains(&scheme.chars().count())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+.^_`|~-".contains(c));
        if !token {
            return Err(field(
                "credential.scheme",
                format!("`{scheme}` must be 1 to {MAX_SCHEME_CHARS} characters of an HTTP token"),
            ));
        }
        AUTHORIZATION.to_owned()
    } else {
        if source != Source::Catalog {
            return Err(field(
                "credential.header",
                format!(
                    "`{header}`: outside the catalog a credential travels only in Authorization; another header needs the catalog's review"
                ),
            ));
        }
        if HOST_HEADERS.contains(&lower.as_str())
            || lower.starts_with("proxy-")
            || lower.starts_with("content-")
        {
            return Err(field(
                "credential.header",
                format!("`{header}` is a header the host sets or forbids"),
            ));
        }
        if scheme.is_some() {
            return Err(field(
                "credential.scheme",
                "only a credential in Authorization has a scheme",
            ));
        }
        header.to_owned()
    };
    for (name, operation) in operations {
        if let Some((fixed, _)) = operation
            .headers
            .iter()
            .find(|(fixed, _)| fixed.eq_ignore_ascii_case(&header))
        {
            return Err(field(
                &format!("operations.{name}.headers"),
                format!("`{fixed}` is the credential's header, which only the host writes"),
            ));
        }
    }
    Ok(header)
}

#[allow(clippy::too_many_arguments)]
fn check_account(
    account: RawAccount,
    destinations: &BTreeMap<String, Origin>,
    origins: &[Origin],
    credential_destinations: &[String],
    operations: &BTreeMap<String, Operation>,
    reserves: &[String],
    source: Source,
) -> Result<AccountDecl, RuntimeError> {
    let RawFlow::AuthorizationCode = account.flow;
    if !is_name(&account.client, MAX_CLIENT_CHARS, '-') {
        return Err(field(
            "credential.account.client",
            format!(
                "`{}` must be a lowercase letter, then up to 31 lowercase letters, digits or hyphens",
                account.client
            ),
        ));
    }
    let scopes = account.scopes.unwrap_or_default();
    if scopes.len() > MAX_SCOPES {
        return Err(field(
            "credential.account.scopes",
            format!("at most {MAX_SCOPES} scopes"),
        ));
    }
    for (index, scope) in scopes.iter().enumerate() {
        // RFC 6749's scope-token: %x21 / %x23-5B / %x5D-7E
        let valid = (1..=MAX_SCOPE_CHARS).contains(&scope.chars().count())
            && scope.chars().all(|c| {
                c == '\u{21}'
                    || ('\u{23}'..='\u{5b}').contains(&c)
                    || ('\u{5d}'..='\u{7e}').contains(&c)
            });
        if !valid {
            return Err(field(
                &format!("credential.account.scopes[{index}]"),
                format!("`{scope}` must be 1 to {MAX_SCOPE_CHARS} characters of an OAuth scope"),
            ));
        }
        if scopes[..index].contains(scope) {
            return Err(field(
                "credential.account.scopes",
                format!("`{scope}` is listed twice"),
            ));
        }
    }
    let access = check_access(account.access, "credential.account.access")?;

    // Where the browser goes: a host the connector reserves (catalog) or declares (development)
    let browser_host = |name: &str, address: &str| -> Result<url::Url, RuntimeError> {
        let url = https_address(name, address)?;
        let host = url.host_str().unwrap_or_default();
        let allowed = match source {
            Source::Catalog => reserves.iter().any(|domain| is_under(host, domain)),
            _ => destinations.values().any(|origin| origin.host == host),
        };
        if !allowed {
            return Err(field(
                name,
                format!(
                    "{host} is not a host the connector {}",
                    if source == Source::Catalog {
                        "reserves"
                    } else {
                        "declares"
                    }
                ),
            ));
        }
        Ok(url)
    };
    // Where a credential goes: one of the credential's own destinations
    let credential_origin = |name: &str, address: &str| -> Result<url::Url, RuntimeError> {
        let url = https_address(name, address)?;
        let port = url.port_or_known_default().unwrap_or(443);
        let host = url.host_str().unwrap_or_default();
        if !origins
            .iter()
            .any(|origin| origin.https && origin.host == host && origin.port == port)
        {
            return Err(field(
                name,
                format!(
                    "{} is not one of the destinations the credential is for, so a refresh token or a token being revoked would reach a host the credential is not for",
                    url.origin().ascii_serialization()
                ),
            ));
        }
        Ok(url)
    };

    let authorize = browser_host("credential.account.authorize", &account.authorize)?;
    let token = credential_origin("credential.account.token", &account.token)?;
    let revoke = account
        .revoke
        .as_deref()
        .map(|address| credential_origin("credential.account.revoke", address))
        .transpose()?;
    let access_page = account
        .access_page
        .as_deref()
        .map(|address| browser_host("credential.account.access_page", address))
        .transpose()?;
    let revoke_page = account
        .revoke_page
        .as_deref()
        .map(|address| browser_host("credential.account.revoke_page", address))
        .transpose()?;

    let identity = match account.identity {
        None => None,
        Some(Object(identity)) => {
            let at = "credential.account.identity";
            let operation = operations.get(&identity.operation).ok_or_else(|| {
                field(
                    at,
                    format!(
                        "`{}` is not an operation of the connector",
                        identity.operation
                    ),
                )
            })?;
            if operation.kind != Kind::Read
                || operation.method != Method::Get
                || !operation.parameters.is_empty()
            {
                return Err(field(
                    at,
                    format!(
                        "`{}` must be a GET read without parameters",
                        identity.operation
                    ),
                ));
            }
            if !credential_destinations.contains(&operation.destination) {
                return Err(field(
                    at,
                    format!(
                        "`{}` goes to `{}`, a destination the credential is not for",
                        identity.operation, operation.destination
                    ),
                ));
            }
            for (name, pointer) in [("name", &identity.name), ("id", &identity.id)] {
                let segments: Vec<&str> = pointer.split('/').skip(1).collect();
                let valid = pointer.starts_with('/')
                    && (1..=MAX_POINTER_SEGMENTS).contains(&segments.len())
                    && segments.iter().all(|segment| {
                        (1..=MAX_POINTER_SEGMENT_CHARS).contains(&segment.chars().count())
                    });
                if !valid {
                    return Err(field(
                        &format!("{at}.{name}"),
                        format!(
                            "`{pointer}` must be a JSON pointer of 1 to {MAX_POINTER_SEGMENTS} parts"
                        ),
                    ));
                }
            }
            Some(Identity {
                operation: identity.operation,
                name: identity.name,
                id: identity.id,
            })
        }
    };

    Ok(AccountDecl {
        client: account.client,
        authorize,
        token,
        revoke,
        scopes,
        access,
        identity,
        access_page,
        revoke_page,
    })
}

/// An absolute `https` address with a DNS name, no user, password or fragment.
fn https_address(name: &str, address: &str) -> Result<url::Url, RuntimeError> {
    if address.chars().count() > MAX_ADDRESS_CHARS {
        return Err(field(
            name,
            format!("longer than {MAX_ADDRESS_CHARS} characters"),
        ));
    }
    let url = url::Url::parse(address)
        .map_err(|error| field(name, format!("`{address}` is not an address: {error}")))?;
    let domain = matches!(url.host(), Some(url::Host::Domain(domain)) if !domain.ends_with('.'));
    if url.scheme() != "https"
        || !domain
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(field(
            name,
            format!("`{address}` must be https://<domain>/…, with no user, password or fragment"),
        ));
    }
    Ok(url)
}

/// A credential field anywhere, at `path` or under it.
fn refuse_credentials(value: &Value, path: &str) -> Result<(), RuntimeError> {
    match value {
        Value::Object(fields) => {
            for (name, item) in fields {
                let here = if path.is_empty() {
                    name.clone()
                } else {
                    format!("{path}.{name}")
                };
                let top_level_credential = path.is_empty() && name == "credential";
                if CREDENTIAL_FIELDS.contains(&name.as_str()) && !top_level_credential {
                    return Err(RuntimeError::new(
                        ErrorKind::CredentialsNotSupported,
                        format!(
                            "{here}: a connector declares its credential only in the top-level `credential` field"
                        ),
                    ));
                }
                if top_level_credential {
                    continue;
                }
                refuse_credentials(item, &here)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                refuse_credentials(item, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// JSON with every object's keys sorted and no whitespace, whatever order a map keeps.
pub(crate) fn canonical(value: &Value) -> String {
    match value {
        Value::Object(fields) => {
            let mut names: Vec<&String> = fields.keys().collect();
            names.sort();
            let items: Vec<String> = names
                .into_iter()
                .map(|name| {
                    format!(
                        "{}:{}",
                        Value::String(name.clone()),
                        canonical(&fields[name])
                    )
                })
                .collect();
            format!("{{{}}}", items.join(","))
        }
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(canonical).collect();
            format!("[{}]", items.join(","))
        }
        other => other.to_string(),
    }
}

/// `sha256:` and the lowercase hex SHA-256 of `bytes`.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity("sha256:".len() + 64);
    text.push_str("sha256:");
    for byte in digest {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

/// JSON read into a [`Value`], refusing a key given twice in any object.
struct Strict(Value);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor).map(Strict)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        Ok(serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number))
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(Strict(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut fields = serde_json::Map::new();
        let mut seen = BTreeSet::new();
        while let Some(name) = map.next_key::<String>()? {
            if !seen.insert(name.clone()) {
                return Err(de::Error::custom(format!("`{name}` is given twice")));
            }
            let Strict(item) = map.next_value()?;
            fields.insert(name, item);
        }
        Ok(Value::Object(fields))
    }
}

fn invalid(detail: impl AsRef<str>) -> RuntimeError {
    RuntimeError::new(ErrorKind::InvalidConnector, detail)
}

/// An `invalid-connector` error naming the field path and the problem.
fn field(path: &str, problem: impl fmt::Display) -> RuntimeError {
    invalid(format!("{path}: {problem}"))
}

/// A parse error at `path`, which is `.` for the whole document.
fn field_error(path: &str, error: &serde_json::Error) -> RuntimeError {
    if path == "." {
        invalid(error.to_string())
    } else {
        field(path, error)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

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
        let long = format!("local/{}", "a".repeat(65));
        assert_invalid_local(|c| c["id"] = json!(long), &["id"]);
        assert!(parse_local(|c| c["id"] = json!(format!("local/{}", "a".repeat(64)))).is_ok());
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
            "HTTPS://api.weather.test",
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
            c["destinations"]["api"] = json!("https://API.Weather.TEST:443");
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
    fn a_credential_outside_the_credential_field_is_refused() {
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
            assert!(error.detail().contains("`credential`"), "{error}");
        }
    }

    /// The credential of `specs/005-plugin-credentials/data-model.md` (Connector: `credential`):
    /// an account on the connector's only destination, with an identity operation.
    fn with_account(connector: &mut Value) {
        connector["operations"]["get-me"] = json!({
            "kind": "read", "method": "GET", "destination": "api", "path": "/me",
            "response": { "format": "json" }
        });
        connector["credential"] = json!({
            "kind": "account",
            "required": false,
            "destinations": ["api"],
            "header": "Authorization",
            "scheme": "Bearer",
            "account": {
                "flow": "authorization-code",
                "client": "weather",
                "authorize": "https://api.weather.test/oauth/authorize",
                "token": "https://api.weather.test/oauth/token",
                "revoke": "https://api.weather.test/oauth/revoke",
                "scopes": ["forecast:read"],
                "access": [{ "name": "forecasts", "level": "read" }],
                "identity": { "operation": "get-me", "name": "/name", "id": "/id" }
            },
            "subscription_required": [402]
        });
    }

    fn parse_account(edit: impl FnOnce(&mut Value)) -> Result<Connector, RuntimeError> {
        parse_local(|c| {
            with_account(c);
            edit(c);
        })
    }

    #[track_caller]
    fn assert_invalid_account(edit: impl FnOnce(&mut Value), parts: &[&str]) {
        assert_invalid(parse_account(edit), parts);
    }

    #[test]
    fn an_account_credential_is_read_with_every_field() {
        let connector = parse_account(|_| {}).unwrap();
        let credential = connector.credential().expect("a credential");
        assert_eq!(credential.kind(), CredentialKind::Account);
        assert!(!credential.required());
        assert_eq!(
            credential
                .origins()
                .iter()
                .map(|o| o.serialization())
                .collect::<Vec<_>>(),
            ["https://api.weather.test"]
        );
        assert_eq!(credential.header(), "Authorization");
        assert_eq!(credential.header_value("t0k"), "Bearer t0k");
        let account = credential.account().expect("an account");
        assert_eq!(account.client(), "weather");
        assert_eq!(
            account.authorize().as_str(),
            "https://api.weather.test/oauth/authorize"
        );
        assert_eq!(
            account.token().as_str(),
            "https://api.weather.test/oauth/token"
        );
        assert_eq!(
            account.revoke().map(url::Url::as_str),
            Some("https://api.weather.test/oauth/revoke")
        );
        assert_eq!(account.scopes(), ["forecast:read"]);
        let identity = account.identity().expect("an identity");
        assert_eq!(identity.operation(), "get-me");
        assert_eq!((identity.name(), identity.id()), ("/name", "/id"));
        assert_eq!(
            credential.access(),
            [Access {
                name: "forecasts".to_owned(),
                level: Level::Read
            }]
        );
        assert_eq!(credential.subscription_required(), [402]);
        // A key has no account, and access only when declared
        let key = parse_local(|c| {
            c["credential"] = json!({
                "kind": "key", "required": true, "destinations": ["api"],
                "header": "Authorization", "scheme": "Bearer"
            });
        })
        .unwrap();
        let key = key.credential().unwrap();
        assert_eq!(key.kind(), CredentialKind::Key);
        assert!(key.required() && key.account().is_none() && key.access().is_empty());
        // Without a credential, nothing changes from feature 003
        assert!(parse_local(|_| {}).unwrap().credential().is_none());
    }

    #[test]
    fn a_credential_follows_its_schema() {
        assert_invalid_account(
            |c| c["credential"]["kind"] = json!("password"),
            &["credential.kind"],
        );
        assert_invalid_account(
            |c| c["credential"]["account"]["flow"] = json!("device"),
            &["credential.account.flow"],
        );
        // account only with kind: account, and required with it
        assert_invalid_account(
            |c| c["credential"]["kind"] = json!("key"),
            &["credential.account"],
        );
        assert_invalid_account(
            |c| {
                c["credential"].as_object_mut().unwrap().remove("account");
            },
            &["credential.account"],
        );
        assert_invalid_account(
            |c| c["credential"]["account"]["scopes"] = json!(["a\"b"]),
            &["credential.account.scopes"],
        );
        assert_invalid_account(
            |c| c["credential"]["account"]["scopes"] = json!(vec!["s"; 33]),
            &["credential.account.scopes"],
        );
        assert_invalid_account(
            |c| c["credential"]["account"]["access"] = json!([]),
            &["credential.account.access"],
        );
        assert_invalid_account(
            |c| {
                c["credential"]["account"]["access"] =
                    json!([{ "name": "notes", "level": "admin" }])
            },
            &["credential.account.access"],
        );
        assert_invalid_account(
            |c| {
                c["credential"]["account"]["access"] =
                    json!([{ "name": "x".repeat(65), "level": "read" }])
            },
            &["credential.account.access"],
        );
        for status in [399, 500, 200] {
            assert_invalid_account(
                |c| c["credential"]["subscription_required"] = json!([status]),
                &["credential.subscription_required"],
            );
        }
        assert_invalid_account(
            |c| c["credential"]["subscription_required"] = json!([400, 401, 402, 403, 404]),
            &["credential.subscription_required"],
        );
        assert_invalid_account(|c| c["credential"]["unknown"] = json!(1), &["unknown"]);
    }

    #[test]
    fn a_credential_goes_only_to_declared_https_destinations() {
        assert_invalid_account(
            |c| c["credential"]["destinations"] = json!(["nowhere"]),
            &["credential.destinations", "nowhere"],
        );
        assert_invalid_account(
            |c| c["credential"]["destinations"] = json!([]),
            &["credential.destinations"],
        );
        // Every destination of a connector with a credential is HTTPS, not only the
        // credential's own (Principle III): plain-http-beside and plain-http-credential
        for edit in [
            (|c: &mut Value| {
                c["destinations"]["here"] = json!("http://127.0.0.1:9");
            }) as fn(&mut Value),
            |c| {
                c["destinations"]["api"] = json!("http://127.0.0.1:9");
                c["credential"]["account"]["authorize"] = json!("https://login.weather.test/a");
            },
        ] {
            let error = parse_account(edit).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::CredentialNeedsHttps, "{error}");
        }
        // The same destination over plain HTTP, without a credential, is still a local
        // connector of feature 003
        assert!(parse_local(|c| c["destinations"]["here"] = json!("http://127.0.0.1:9")).is_ok());
    }

    #[test]
    fn the_credential_header_belongs_to_the_host() {
        assert_invalid_account(
            |c| {
                c["credential"].as_object_mut().unwrap().remove("scheme");
            },
            &["credential.scheme"],
        );
        assert_invalid_account(
            |c| c["credential"]["scheme"] = json!("Bea rer"),
            &["credential.scheme"],
        );
        // Outside the catalog, only Authorization; never a header the host forbids
        assert_invalid_account(
            |c| {
                c["credential"]["header"] = json!("X-Api-Key");
                c["credential"].as_object_mut().unwrap().remove("scheme");
            },
            &["credential.header"],
        );
        let catalog = parse_catalog(|c| {
            with_account(c);
            c["credential"]["header"] = json!("X-Api-Key");
            c["credential"].as_object_mut().unwrap().remove("scheme");
        })
        .unwrap();
        assert_eq!(catalog.credential().unwrap().header_value("t0k"), "t0k");
        for header in ["Cookie", "Host", "Content-Type"] {
            assert_invalid(
                Connector::parse(
                    &catalog_with(|c| {
                        with_account(c);
                        c["credential"]["header"] = json!(header);
                        c["credential"].as_object_mut().unwrap().remove("scheme");
                    }),
                    Source::Catalog,
                ),
                &["credential.header"],
            );
        }
        // fixed-auth-header: an operation fixing the credential's header, in any case
        assert_invalid(
            Connector::parse(
                &catalog_with(|c| {
                    with_account(c);
                    c["credential"]["header"] = json!("X-Api-Key");
                    c["credential"].as_object_mut().unwrap().remove("scheme");
                    forecast(c)["headers"]["x-api-key"] = json!("fixed");
                }),
                Source::Catalog,
            ),
            &["x-api-key"],
        );
        assert_invalid_account(
            |c| forecast(c)["headers"]["authorization"] = json!("Bearer fixed"),
            &["authorization"],
        );
    }

    #[test]
    fn sign_in_addresses_stay_on_the_connectors_hosts() {
        // token and revoke on the credential's own destinations (token-host-elsewhere)
        for at in ["token", "revoke"] {
            assert_invalid_account(
                |c| {
                    c["destinations"]["other"] = json!("https://other.test");
                    c["credential"]["account"][at] = json!("https://other.test/oauth/x");
                },
                &[&format!("credential.account.{at}")],
            );
        }
        // authorize and the pages on one of the connector's hosts, never anywhere else
        for at in ["authorize", "access_page", "revoke_page"] {
            assert_invalid_account(
                |c| c["credential"]["account"][at] = json!("https://elsewhere.test/x"),
                &[&format!("credential.account.{at}")],
            );
            assert!(
                parse_account(|c| {
                    c["destinations"]["other"] = json!("https://login.weather.test");
                    c["credential"]["account"][at] = json!("https://login.weather.test/x");
                })
                .is_ok(),
                "{at} on another destination of the connector"
            );
        }
        for address in [
            "http://api.weather.test/oauth/token",
            "https://user@api.weather.test/oauth/token",
            "https://api.weather.test/oauth/token#x",
            "/oauth/token",
        ] {
            assert_invalid_account(
                |c| c["credential"]["account"]["token"] = json!(address),
                &["credential.account.token"],
            );
        }
        // A catalog connector: authorize under the domains it reserves
        assert!(
            Connector::parse(
                &catalog_with(|c| {
                    with_account(c);
                    c["credential"]["account"]["authorize"] = json!("https://login.weather.test/a");
                }),
                Source::Catalog
            )
            .is_ok()
        );
        assert_invalid(
            Connector::parse(
                &catalog_with(|c| {
                    with_account(c);
                    c["credential"]["account"]["authorize"] = json!("https://weather.example/a");
                }),
                Source::Catalog,
            ),
            &["credential.account.authorize"],
        );
    }

    #[test]
    fn the_identity_operation_is_a_parameterless_read_on_a_credential_destination() {
        // identity-elsewhere
        assert_invalid_account(
            |c| {
                c["destinations"]["other"] = json!("https://other.test");
                c["operations"]["get-me"]["destination"] = json!("other");
            },
            &["credential.account.identity"],
        );
        assert_invalid_account(
            |c| c["credential"]["account"]["identity"]["operation"] = json!("missing"),
            &["credential.account.identity"],
        );
        assert_invalid_account(
            |c| c["credential"]["account"]["identity"]["operation"] = json!("forecast"),
            &["credential.account.identity"],
        );
        assert_invalid_account(
            |c| {
                c["credential"]["account"]["identity"]
                    .as_object_mut()
                    .unwrap()
                    .remove("id");
            },
            &["credential.account.identity"],
        );
        // Without identity the account is named `account` and cannot be compared
        let connector = parse_account(|c| {
            c["credential"]["account"]
                .as_object_mut()
                .unwrap()
                .remove("identity");
        })
        .unwrap();
        assert!(
            connector
                .credential()
                .unwrap()
                .account()
                .unwrap()
                .identity()
                .is_none()
        );
    }

    #[test]
    fn the_credential_digest_leaves_out_access_and_follows_the_rest() {
        let digest =
            |edit: fn(&mut Value)| parse_account(edit).unwrap().credential().unwrap().digest();
        let base = digest(|_| {});
        assert!(base.starts_with("sha256:"));
        assert_eq!(
            base,
            digest(|c| c["credential"]["account"]["access"] =
                json!([{ "name": "forecasts", "level": "write" }]))
        );
        for edit in [
            (|c: &mut Value| c["credential"]["required"] = json!(true)) as fn(&mut Value),
            |c| c["credential"]["scheme"] = json!("Token"),
            |c| {
                c["credential"]["account"]["token"] = json!("https://api.weather.test/oauth/token2")
            },
            |c| c["credential"]["account"]["scopes"] = json!(["forecast:read", "x"]),
            |c| c["credential"]["subscription_required"] = json!([]),
            |c| c["credential"]["account"]["identity"]["name"] = json!("/login"),
        ] {
            assert_ne!(base, digest(edit));
        }
        // The operations' digests do not change when a credential is added
        assert_eq!(
            parse_local(|_| {}).unwrap().operation_digest("forecast"),
            parse_account(|_| {}).unwrap().operation_digest("forecast")
        );
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
