//! What the installation dialog shows (`specs/010-plugin-installation/data-model.md`, Dialog
//! model; research R6, R7): built by the host from a checked package, never from the plugin's
//! words, and drawn by the page as it is. Every permission has a sentence the host wrote; every
//! connector is shown as the development tool shows it; the warning comes from
//! [`flows::judge`](crate::flows::judge); and two digests say exactly what an approval covers.

use serde::Serialize;
use serde_json::json;

use crate::connector::{Connector, Kind, Operation, ParameterType, Template, is_placeholder};
use crate::flows::{Flow, judge};
use crate::{
    CheckedPackage, LoadedConnector, PermissionKind, drawn_name, drawn_text, escape, package,
};

/// The longest author the dialog draws.
pub const DRAWN_AUTHOR_CHARS: usize = 60;

/// What the dialog is asked to decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DialogKind {
    Install,
    /// An update that needs a new approval.
    Update,
    /// An update that needs nothing new: the person confirms it.
    UpdateConfirm,
}

/// Who is asking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Asking {
    /// The manifest's name as the frame draws it, or the end of the id.
    pub name: String,
    /// The repository id, in full.
    pub id: String,
    /// The manifest's author, sanitised and bounded; the page labels it as declared by the
    /// plugin.
    pub author: String,
    pub version: String,
    pub installed_version: Option<String>,
}

/// What an update changes of one permission or connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Change {
    Added,
    Changed,
    Unchanged,
}

/// One permission, in the host's words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PermissionLine {
    /// The manifest's name for it, so the page can send it back as a choice.
    pub name: String,
    pub sentence: String,
    pub required: bool,
    pub change: Option<Change>,
}

/// One operation the widget calls, as the development tool shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperationLine {
    pub name: String,
    /// `read` or `change`.
    pub kind: &'static str,
    /// `reviewed` (the catalog's review) or `declared` (the connector's author's word).
    pub review: &'static str,
    pub method: &'static str,
    /// The destination and the path, `{parameter}` where a value goes.
    pub url: String,
    /// What it sends: `name (text)`, `name (integer, optional)`; empty when nothing.
    pub sends: Vec<String>,
    /// Fixed query values, headers and body fields.
    pub fixed: Vec<String>,
}

/// The account a connector would act as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountLine {
    pub sentence: String,
    /// `notes (read)`, sorted.
    pub access: Vec<String>,
    /// Always false in this feature: the app connects no account yet (FR-024).
    pub connected: bool,
}

/// One connector, as the development tool shows it, with the host's sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectorLine {
    pub id: String,
    pub version: String,
    /// `catalog`: only catalog connectors install (FR-015).
    pub origin: String,
    pub required: bool,
    pub change: Option<Change>,
    pub sentence: String,
    pub destinations: Vec<String>,
    pub account: Option<AccountLine>,
    pub operations: Vec<OperationLine>,
}

/// Everything the dialog shows (`data-model.md`, Dialog model).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DialogModel {
    pub kind: DialogKind,
    pub asking: Asking,
    pub permissions: Vec<PermissionLine>,
    pub connectors: Vec<ConnectorLine>,
    /// Empty: no warning.
    pub warning: Vec<Flow>,
    /// The approval digest: package, permissions, connectors, storage.
    pub digest: String,
    /// The choice digest: the approval digest, the optionals turned on and the warning shown,
    /// which `request_approve` must echo.
    pub choice: String,
}

/// What to build a model of.
pub struct ModelInput<'a> {
    pub kind: DialogKind,
    pub package: &'a CheckedPackage,
    /// The installed version's package, for an update: what changed is marked against it.
    pub previous: Option<&'a CheckedPackage>,
    /// The optionals the person turned on, by their manifest names.
    pub optional_on: &'a [String],
    /// The account sources the installed version's store may hold (`Entry::account_sources`).
    pub remembered: &'a [String],
    /// For an update that is only confirmed: the optional connectors the installed version has
    /// on, which stay on, so the warning is judged with them (the review of task T053).
    pub kept_on: &'a [String],
}

/// The dialog's model of `input`.
pub fn model(input: &ModelInput<'_>) -> DialogModel {
    let package = input.package;
    let manifest = package.manifest();
    let on = |name: &str| input.optional_on.iter().any(|optional| optional == name);
    let previous = input.previous;

    let permissions = manifest
        .permissions()
        .iter()
        .filter_map(|request| {
            let kind = PermissionKind::from_name(&request.name)?;
            let quota = |of: &crate::manifest::Manifest| {
                (kind == PermissionKind::Storage)
                    .then(|| of.storage().map(|storage| storage.quota_bytes))
                    .flatten()
            };
            let change =
                previous.map(
                    |previous| match previous.manifest().permission(&request.name) {
                        None => Change::Added,
                        Some(before)
                            if before.required != request.required
                                || quota(previous.manifest()) != quota(manifest) =>
                        {
                            Change::Changed
                        }
                        Some(_) => Change::Unchanged,
                    },
                );
            Some(PermissionLine {
                name: request.name.clone(),
                sentence: permission_sentence(kind, quota(manifest)),
                required: request.required,
                change,
            })
        })
        .collect();

    let connectors = package
        .connectors()
        .iter()
        .map(|loaded| {
            let uses_account = manifest
                .connector(loaded.id())
                .is_some_and(|request| request.account);
            let change = previous.map(|previous| {
                match previous
                    .connectors()
                    .iter()
                    .find(|before| before.id() == loaded.id())
                {
                    None => Change::Added,
                    Some(before) if shape(before) != shape(loaded) => Change::Changed,
                    Some(_) => Change::Unchanged,
                }
            });
            connector_line(loaded, uses_account, change)
        })
        .collect();

    let granted = |id: &str| {
        manifest.connector(id).is_some_and(|request| {
            request.required || on(id) || input.kept_on.iter().any(|kept| kept == id)
        })
    };
    let warning = judge(package, &granted, input.remembered);
    let digest = package.approval_digest();
    let choice = choice_digest(&digest, input.optional_on, &warning);
    DialogModel {
        kind: input.kind,
        asking: Asking {
            name: drawn_name(manifest.name(), manifest.id().as_str()),
            id: manifest.id().to_string(),
            author: drawn_text(manifest.author(), DRAWN_AUTHOR_CHARS),
            version: manifest.version().to_owned(),
            installed_version: previous.map(|previous| previous.manifest().version().to_owned()),
        },
        permissions,
        connectors,
        warning,
        digest,
        choice,
    }
}

/// The digest of what an approval covers in the dialog: the approval digest, the optionals
/// turned on (sorted, each once) and the warning shown for them (research R7).
pub fn choice_digest(approval_digest: &str, optional_on: &[String], warning: &[Flow]) -> String {
    let mut optional: Vec<&String> = optional_on.iter().collect();
    optional.sort();
    optional.dedup();
    let canonical = json!({
        "digest": approval_digest,
        "optional_on": optional,
        "warning": warning,
    });
    package::digest(canonical.to_string().as_bytes())
}

/// The host's sentence for a permission (FR-011). A permission without one does not compile.
pub fn permission_sentence(kind: PermissionKind, quota_bytes: Option<u64>) -> String {
    match kind {
        PermissionKind::Log => "Escrever mensagens no registro do app, que só você vê.".to_owned(),
        PermissionKind::Storage => format!(
            "Guardar dados neste computador, até {}.",
            human_bytes(quota_bytes.unwrap_or_default())
        ),
    }
}

/// `64 KiB`, `8 MiB`, `100 bytes`.
pub fn human_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    if bytes >= MIB && bytes.is_multiple_of(MIB) {
        format!("{} MiB", bytes / MIB)
    } else if bytes >= KIB && bytes.is_multiple_of(KIB) {
        format!("{} KiB", bytes / KIB)
    } else {
        format!("{bytes} bytes")
    }
}

/// What an update compares of a connector: what its approval covers.
#[derive(PartialEq, Eq)]
struct Shape {
    origin: String,
    required: bool,
    destinations: Vec<String>,
    operations: Vec<(String, Option<String>)>,
    credential: bool,
}

fn shape(loaded: &LoadedConnector) -> Shape {
    let connector = loaded.connector();
    Shape {
        origin: loaded.origin().to_owned(),
        required: loaded.required(),
        destinations: connector
            .destinations()
            .iter()
            .map(|origin| origin.serialization())
            .collect(),
        operations: loaded
            .operations()
            .iter()
            .map(|name| (name.clone(), connector.operation_digest(name)))
            .collect(),
        credential: connector.credential().is_some(),
    }
}

fn connector_line(
    loaded: &LoadedConnector,
    uses_account: bool,
    change: Option<Change>,
) -> ConnectorLine {
    let connector = loaded.connector();
    let catalog = loaded.origin() == "catalog";
    let destinations: Vec<String> = connector
        .destinations()
        .iter()
        .map(|origin| escape(&origin.serialization()))
        .collect();
    let service: Vec<String> = connector
        .destinations()
        .iter()
        .map(|origin| escape(origin.host()))
        .collect();
    let service = service.join(", ");
    let operations: Vec<OperationLine> = loaded
        .operations()
        .iter()
        .filter_map(|name| {
            let operation = connector.operation(name)?;
            Some(OperationLine {
                name: escape(name),
                kind: match operation.kind() {
                    Kind::Read => "read",
                    Kind::Change => "change",
                },
                review: if catalog { "reviewed" } else { "declared" },
                method: operation.method().as_str(),
                url: operation_url(connector, operation),
                sends: sends(operation),
                fixed: fixed(operation),
            })
        })
        .collect();
    let reads: Vec<&str> = operations
        .iter()
        .filter(|operation| operation.kind == "read")
        .map(|operation| operation.name.as_str())
        .collect();
    let changes: Vec<&str> = operations
        .iter()
        .filter(|operation| operation.kind == "change")
        .map(|operation| operation.name.as_str())
        .collect();
    let mut sentence = Vec::new();
    if !reads.is_empty() {
        sentence.push(format!("Consultar {service}: {}.", reads.join(", ")));
    }
    if !changes.is_empty() {
        sentence.push(format!(
            "Alterar dados em {service}: {}.",
            changes.join(", ")
        ));
    }
    let account = connector
        .credential()
        .filter(|_| uses_account)
        .map(|credential| {
            let mut access: Vec<String> = credential
                .access()
                .iter()
                .map(|access| {
                    let level = match access.level {
                        crate::connector::Level::Read => "read",
                        crate::connector::Level::Write => "write",
                    };
                    format!("{} ({level})", escape(&access.name))
                })
                .collect();
            access.sort();
            let shown = if access.is_empty() {
                "nenhum declarado".to_owned()
            } else {
                access.join(", ")
            };
            AccountLine {
                sentence: format!(
                    "Ler os dados da sua conta em {service}, com o acesso: {shown}. Nenhuma conta está conectada no app ainda."
                ),
                access,
                connected: false,
            }
        });
    ConnectorLine {
        id: escape(loaded.id()),
        version: escape(&connector.version().to_string()),
        origin: if catalog {
            "catalog".to_owned()
        } else {
            escape(loaded.origin())
        },
        required: loaded.required(),
        change,
        sentence: sentence.join(" "),
        destinations,
        account,
        operations,
    }
}

/// The destination and the path of an operation, `{parameter}` where a value goes.
pub fn operation_url(connector: &Connector, operation: &Operation) -> String {
    let origin = connector
        .destination(operation.destination())
        .map(|origin| origin.serialization())
        .unwrap_or_default();
    let mut path = String::new();
    for segment in operation.path() {
        path.push('/');
        match segment {
            Template::Literal(text) => path.push_str(text),
            Template::Parameter(parameter) => path.push_str(&format!("{{{parameter}}}")),
        }
    }
    escape(&format!("{origin}{path}"))
}

/// The parameters an operation sends: those in its path first, in order, then the rest by name.
pub fn sends(operation: &Operation) -> Vec<String> {
    let parameters = operation.parameters();
    let mut names: Vec<&String> = operation
        .path()
        .iter()
        .filter_map(|segment| match segment {
            Template::Parameter(name) => parameters.get_key_value(name).map(|(name, _)| name),
            Template::Literal(_) => None,
        })
        .collect();
    for name in parameters.keys() {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
        .into_iter()
        .map(|name| {
            let parameter = &parameters[name];
            let kind = match parameter.kind {
                ParameterType::Text { .. } => "text",
                ParameterType::Integer { .. } => "integer",
                ParameterType::Flag => "flag",
            };
            if parameter.required {
                format!("{} ({kind})", escape(name))
            } else {
                format!("{} ({kind}, optional)", escape(name))
            }
        })
        .collect()
}

/// The fixed query values, headers and body fields of an operation.
pub fn fixed(operation: &Operation) -> Vec<String> {
    let mut fixed: Vec<String> = operation
        .query()
        .iter()
        .filter_map(|(name, value)| match value {
            Template::Literal(value) => Some(format!("{}={}", escape(name), escape(value))),
            Template::Parameter(_) => None,
        })
        .collect();
    fixed.extend(
        operation
            .headers()
            .iter()
            .map(|(name, value)| format!("{}: {}", escape(name), escape(value))),
    );
    if let Some(body) = operation.body() {
        fixed_body(body, "body", &mut fixed);
    }
    fixed
}

fn fixed_body(value: &serde_json::Value, path: &str, fixed: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (name, item) in fields {
                fixed_body(item, &format!("{path}.{name}"), fixed);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                fixed_body(item, &format!("{path}[{index}]"), fixed);
            }
        }
        serde_json::Value::String(text) if is_placeholder(text) => {}
        other => fixed.push(format!("{}: {}", escape(path), escape(&other.to_string()))),
    }
}
