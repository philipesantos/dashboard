//! Widget content: what a widget's `render` returns, read by the host
//! (`specs/009-declarative-widget-ui/contracts/content-format.md`).
//!
//! [`parse`] checks a document against the format and its bounds and returns either a
//! normalised tree, with only the fields the host draws and every tone and variant resolved, or
//! the reason the document cannot be drawn. The page draws only the normalised tree, never the
//! plugin's JSON. The published schema is `wit/content.schema.json`; the tests in
//! `tests/schema.rs` keep the two in agreement.
//!
//! The schema refuses unknown fields, tones, variants and component types, so an author's tools
//! catch them. The host is lenient there only, as criterion HS-09 requires: an unknown field is
//! dropped, an unknown tone or variant draws as the default, an unknown component type draws as
//! the host's placeholder, and each adds an entry to the plugin's log.
//!
//! Part of the security core (constitution, Principle IX).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::Serialize;
use serde_json::{Map, Value};

/// The version of the format this host reads.
pub const FORMAT: u64 = 1;
/// The bounds of research R5, published in the schema and the author docs.
pub const MAX_DEPTH: usize = 8;
pub const MAX_COMPONENTS: usize = 500;
pub const MAX_STRING_CHARS: usize = 2000;
pub const MAX_LIST_ITEMS: usize = 200;
pub const MAX_TABLE_COLUMNS: usize = 20;
pub const MAX_TABLE_ROWS: usize = 200;
pub const MAX_SERIES: usize = 5;
pub const MAX_POINTS: usize = 200;
pub const MAX_FIELDS: usize = 20;
pub const MAX_OPTIONS: usize = 50;
/// A form field's id: the key of its value in the action's input.
pub const MAX_FIELD_ID_CHARS: usize = 32;

/// The widget map of feature 007, generated from `DESIGN.md`: the tones and variants widget
/// content may name, and the icon each status tone draws.
const WIDGET_MAP: &str = include_str!("../../design/widget-map.generated.json");

struct WidgetMap {
    default_tone: String,
    /// Each tone, with the icon the host draws beside it, if any.
    tones: Vec<(String, Option<String>)>,
    /// `component.variant` names.
    variants: BTreeSet<String>,
}

fn widget_map() -> &'static WidgetMap {
    static MAP: OnceLock<WidgetMap> = OnceLock::new();
    MAP.get_or_init(|| {
        let map: Value = serde_json::from_str(WIDGET_MAP).expect("the widget map is JSON");
        let tones = map["tones"]
            .as_object()
            .expect("the widget map has tones")
            .iter()
            .map(|(name, tone)| (name.clone(), tone["icon"].as_str().map(str::to_owned)))
            .collect();
        let variants = map["variants"]
            .as_object()
            .expect("the widget map has variants")
            .keys()
            .cloned()
            .collect();
        WidgetMap {
            default_tone: map["unknownTone"]
                .as_str()
                .expect("the widget map names its default tone")
                .to_owned(),
            tones,
            variants,
        }
    })
}

/// A tone, resolved, with the icon a status tone draws (HS-08).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tone {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// Normalised widget content: what the page draws.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Content {
    pub body: Vec<Component>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Component {
    Text {
        text: String,
        variant: String,
        tone: Tone,
    },
    List {
        items: Vec<Item>,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Chart {
        kind: String,
        labels: Vec<String>,
        series: Vec<Series>,
    },
    Button {
        label: String,
        action: String,
        variant: String,
    },
    Form {
        fields: Vec<Field>,
        submit: Submit,
    },
    Stack {
        children: Vec<Component>,
    },
    /// A component type this host does not know, drawn as the host's placeholder.
    Placeholder {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub text: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Series {
    pub name: String,
    pub values: Vec<f64>,
}

/// A form field: text, a decimal number, a checkbox, or a choice among options. There is no
/// password, masked, date, file or link kind (HS-06, HS-25).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    pub id: String,
    pub label: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Submit {
    pub label: String,
    pub action: String,
}

/// Why content cannot be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorKind {
    /// A wrong type, a missing field, a value outside its set, or `format` not 1.
    BadShape,
    /// A bound passed.
    OverBound,
}

/// Why content cannot be drawn: its kind, where, and a short host-written reason that never
/// quotes the plugin's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContentError {
    pub kind: ErrorKind,
    pub path: String,
    pub reason: String,
}

impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self.kind {
            ErrorKind::BadShape => "bad-shape",
            ErrorKind::OverBound => "over-bound",
        };
        write!(f, "{kind} at {}: {}", self.path, self.reason)
    }
}

impl std::error::Error for ContentError {}

/// Reads widget content: the normalised tree and the entries for the plugin's log, or why it
/// cannot be drawn.
pub fn parse(document: &Value) -> Result<(Content, Vec<String>), ContentError> {
    let mut reader = Reader::default();
    let object = reader.object(document, "$")?;
    reader.known(object, "$", &["format", "body"]);
    match object.get("format").and_then(Value::as_u64) {
        Some(FORMAT) => {}
        _ => return Err(shape("$.format", "must be 1")),
    }
    let body = reader.components(object.get("body"), "$.body", 1)?;
    if body.is_empty() {
        return Err(shape("$.body", "needs at least one component"));
    }
    let mut log = Vec::new();
    if !reader.unknown_fields.is_empty() {
        log.push(format!(
            "widget content: ignored fields this host does not know: {}",
            reader.unknown_fields.join(", ")
        ));
    }
    log.extend(reader.log);
    Ok((Content { body }, log))
}

fn shape(path: &str, reason: impl Into<String>) -> ContentError {
    ContentError {
        kind: ErrorKind::BadShape,
        path: path.to_owned(),
        reason: reason.into(),
    }
}

fn over(path: &str, reason: impl Into<String>) -> ContentError {
    ContentError {
        kind: ErrorKind::OverBound,
        path: path.to_owned(),
        reason: reason.into(),
    }
}

#[derive(Default)]
struct Reader {
    components: usize,
    unknown_fields: Vec<String>,
    log: Vec<String>,
}

impl Reader {
    fn object<'a>(
        &self,
        value: &'a Value,
        path: &str,
    ) -> Result<&'a Map<String, Value>, ContentError> {
        value
            .as_object()
            .ok_or_else(|| shape(path, "must be an object"))
    }

    /// Notes every field of `object` that is not in `fields`, so the document's log names them.
    fn known(&mut self, object: &Map<String, Value>, path: &str, fields: &[&str]) {
        for name in object.keys() {
            if !fields.contains(&name.as_str()) {
                // The name is the plugin's: only its length is bounded, and it is quoted as data
                let name: String = name.chars().take(64).collect();
                self.unknown_fields.push(format!("{path}.{name:?}"));
            }
        }
    }

    fn string(
        &self,
        object: &Map<String, Value>,
        field: &str,
        path: &str,
    ) -> Result<String, ContentError> {
        let path = format!("{path}.{field}");
        let text = object
            .get(field)
            .ok_or_else(|| shape(&path, "is missing"))?
            .as_str()
            .ok_or_else(|| shape(&path, "must be a string"))?;
        bounded(text, &path)
    }

    fn tone(&mut self, value: Option<&Value>, path: &str) -> Result<Tone, ContentError> {
        let map = widget_map();
        let name = match value {
            None => map.default_tone.clone(),
            Some(Value::String(name)) => {
                if map.tones.iter().any(|(known, _)| known == name) {
                    name.clone()
                } else {
                    let shown: String = name.chars().take(64).collect();
                    self.log.push(format!(
                        "widget content: the tone {shown:?} at {path} is not one this host knows; drawn as {:?}",
                        map.default_tone
                    ));
                    map.default_tone.clone()
                }
            }
            Some(_) => return Err(shape(path, "must be a string")),
        };
        let icon = map
            .tones
            .iter()
            .find(|(known, _)| *known == name)
            .and_then(|(_, icon)| icon.clone());
        Ok(Tone { name, icon })
    }

    /// The variant of `component`, or `default_variant` when absent or unknown (logged).
    fn variant(
        &mut self,
        component: &str,
        value: Option<&Value>,
        default_variant: &str,
        path: &str,
    ) -> Result<String, ContentError> {
        match value {
            None => Ok(default_variant.to_owned()),
            Some(Value::String(name)) => {
                if widget_map()
                    .variants
                    .contains(&format!("{component}.{name}"))
                {
                    Ok(name.clone())
                } else {
                    let shown: String = name.chars().take(64).collect();
                    self.log.push(format!(
                        "widget content: the {component} variant {shown:?} at {path} is not one this host knows; drawn as {default_variant:?}"
                    ));
                    Ok(default_variant.to_owned())
                }
            }
            Some(_) => Err(shape(path, "must be a string")),
        }
    }

    fn components(
        &mut self,
        value: Option<&Value>,
        path: &str,
        depth: usize,
    ) -> Result<Vec<Component>, ContentError> {
        if depth > MAX_DEPTH {
            return Err(over(path, format!("nested deeper than {MAX_DEPTH}")));
        }
        let list = value
            .ok_or_else(|| shape(path, "is missing"))?
            .as_array()
            .ok_or_else(|| shape(path, "must be a list"))?;
        let mut components = Vec::with_capacity(list.len());
        for (index, value) in list.iter().enumerate() {
            components.push(self.component(value, &format!("{path}[{index}]"), depth)?);
        }
        Ok(components)
    }

    fn component(
        &mut self,
        value: &Value,
        path: &str,
        depth: usize,
    ) -> Result<Component, ContentError> {
        self.components += 1;
        if self.components > MAX_COMPONENTS {
            return Err(over(path, format!("more than {MAX_COMPONENTS} components")));
        }
        let object = self.object(value, path)?;
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| shape(&format!("{path}.type"), "must be a string"))?;
        Ok(match kind {
            "text" => {
                self.known(object, path, &["type", "text", "variant", "tone"]);
                Component::Text {
                    text: self.string(object, "text", path)?,
                    variant: self.variant(
                        "text",
                        object.get("variant"),
                        "default",
                        &format!("{path}.variant"),
                    )?,
                    tone: self.tone(object.get("tone"), &format!("{path}.tone"))?,
                }
            }
            "list" => {
                self.known(object, path, &["type", "items"]);
                let items_path = format!("{path}.items");
                let items = list_of(object.get("items"), &items_path, MAX_LIST_ITEMS)?;
                let mut out = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    let item_path = format!("{items_path}[{index}]");
                    out.push(match item {
                        Value::String(text) => Item {
                            text: bounded(text, &item_path)?,
                            tone: self.tone(None, &item_path)?,
                        },
                        Value::Object(item) => {
                            self.known(item, &item_path, &["text", "tone"]);
                            Item {
                                text: self.string(item, "text", &item_path)?,
                                tone: self.tone(item.get("tone"), &format!("{item_path}.tone"))?,
                            }
                        }
                        _ => return Err(shape(&item_path, "must be a string or an object")),
                    });
                }
                Component::List { items: out }
            }
            "table" => {
                self.known(object, path, &["type", "columns", "rows"]);
                let columns = strings(
                    object.get("columns"),
                    &format!("{path}.columns"),
                    MAX_TABLE_COLUMNS,
                )?;
                if columns.is_empty() {
                    return Err(shape(
                        &format!("{path}.columns"),
                        "needs at least one column",
                    ));
                }
                let rows_path = format!("{path}.rows");
                let raw = list_of(object.get("rows"), &rows_path, MAX_TABLE_ROWS)?;
                let mut rows = Vec::with_capacity(raw.len());
                for (index, row) in raw.iter().enumerate() {
                    let row_path = format!("{rows_path}[{index}]");
                    let cells = strings(Some(row), &row_path, MAX_TABLE_COLUMNS)?;
                    if cells.len() != columns.len() {
                        return Err(shape(
                            &row_path,
                            format!("has {} cells for {} columns", cells.len(), columns.len()),
                        ));
                    }
                    rows.push(cells);
                }
                Component::Table { columns, rows }
            }
            "chart" => {
                self.known(object, path, &["type", "kind", "labels", "series"]);
                let kind = match object.get("kind") {
                    None => "bar".to_owned(),
                    Some(Value::String(kind)) if kind == "bar" || kind == "line" => kind.clone(),
                    Some(_) => {
                        return Err(shape(
                            &format!("{path}.kind"),
                            "must be \"bar\" or \"line\"",
                        ));
                    }
                };
                let labels = strings(object.get("labels"), &format!("{path}.labels"), MAX_POINTS)?;
                let series_path = format!("{path}.series");
                let raw = list_of(object.get("series"), &series_path, MAX_SERIES)?;
                if raw.is_empty() {
                    return Err(shape(&series_path, "needs at least one series"));
                }
                let mut series = Vec::with_capacity(raw.len());
                for (index, entry) in raw.iter().enumerate() {
                    let entry_path = format!("{series_path}[{index}]");
                    let entry = self.object(entry, &entry_path)?;
                    self.known(entry, &entry_path, &["name", "values"]);
                    let values_path = format!("{entry_path}.values");
                    let values = list_of(entry.get("values"), &values_path, MAX_POINTS)?
                        .iter()
                        .enumerate()
                        .map(|(point, value)| {
                            value
                                .as_f64()
                                .filter(|number| number.is_finite())
                                .ok_or_else(|| {
                                    shape(&format!("{values_path}[{point}]"), "must be a number")
                                })
                        })
                        .collect::<Result<Vec<f64>, ContentError>>()?;
                    if values.len() != labels.len() {
                        return Err(shape(
                            &values_path,
                            format!("has {} values for {} labels", values.len(), labels.len()),
                        ));
                    }
                    series.push(Series {
                        name: self.string(entry, "name", &entry_path)?,
                        values,
                    });
                }
                Component::Chart {
                    kind,
                    labels,
                    series,
                }
            }
            "button" => {
                self.known(object, path, &["type", "label", "action", "variant"]);
                Component::Button {
                    label: self.string(object, "label", path)?,
                    action: self.string(object, "action", path)?,
                    variant: self.variant(
                        "button",
                        object.get("variant"),
                        "primary",
                        &format!("{path}.variant"),
                    )?,
                }
            }
            "form" => {
                self.known(object, path, &["type", "fields", "submit"]);
                let fields_path = format!("{path}.fields");
                let raw = list_of(object.get("fields"), &fields_path, MAX_FIELDS)?;
                if raw.is_empty() {
                    return Err(shape(&fields_path, "needs at least one field"));
                }
                let mut fields: Vec<Field> = Vec::with_capacity(raw.len());
                for (index, field) in raw.iter().enumerate() {
                    let field_path = format!("{fields_path}[{index}]");
                    let field = self.object(field, &field_path)?;
                    self.known(field, &field_path, &["id", "label", "kind", "options"]);
                    let id = self.string(field, "id", &field_path)?;
                    let well_formed = id.chars().count() <= MAX_FIELD_ID_CHARS
                        && id.starts_with(|c: char| c.is_ascii_lowercase())
                        && id.chars().all(|c| {
                            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'
                        });
                    if !well_formed {
                        return Err(shape(
                            &format!("{field_path}.id"),
                            "must be a lowercase id of at most 32 characters",
                        ));
                    }
                    if fields.iter().any(|other| other.id == id) {
                        return Err(shape(
                            &format!("{field_path}.id"),
                            "repeats another field's id",
                        ));
                    }
                    // No password, masked, date, file or link kind exists (HS-06, HS-25)
                    let kind = self.string(field, "kind", &field_path)?;
                    let options = match kind.as_str() {
                        "text" | "number" | "checkbox" => {
                            if field.contains_key("options") {
                                return Err(shape(
                                    &format!("{field_path}.options"),
                                    "belongs only to a select",
                                ));
                            }
                            Vec::new()
                        }
                        "select" => {
                            let options = strings(
                                field.get("options"),
                                &format!("{field_path}.options"),
                                MAX_OPTIONS,
                            )?;
                            if options.is_empty() {
                                return Err(shape(
                                    &format!("{field_path}.options"),
                                    "needs at least one option",
                                ));
                            }
                            options
                        }
                        _ => {
                            return Err(shape(
                                &format!("{field_path}.kind"),
                                "must be \"text\", \"number\", \"checkbox\" or \"select\"",
                            ));
                        }
                    };
                    fields.push(Field {
                        label: self.string(field, "label", &field_path)?,
                        id,
                        kind,
                        options,
                    });
                }
                let submit_path = format!("{path}.submit");
                let submit = self.object(
                    object
                        .get("submit")
                        .ok_or_else(|| shape(&submit_path, "is missing"))?,
                    &submit_path,
                )?;
                self.known(submit, &submit_path, &["label", "action"]);
                Component::Form {
                    fields,
                    submit: Submit {
                        label: self.string(submit, "label", &submit_path)?,
                        action: self.string(submit, "action", &submit_path)?,
                    },
                }
            }
            "stack" => {
                self.known(object, path, &["type", "children"]);
                Component::Stack {
                    children: self.components(
                        object.get("children"),
                        &format!("{path}.children"),
                        depth + 1,
                    )?,
                }
            }
            other => {
                let name: String = other.chars().take(64).collect();
                self.log.push(format!(
                    "widget content: the component type {name:?} at {path} is not one this host knows; drawn as a placeholder"
                ));
                Component::Placeholder { name }
            }
        })
    }
}

fn bounded(text: &str, path: &str) -> Result<String, ContentError> {
    if text.chars().count() > MAX_STRING_CHARS {
        return Err(over(
            path,
            format!("longer than {MAX_STRING_CHARS} characters"),
        ));
    }
    Ok(text.to_owned())
}

fn list_of<'a>(
    value: Option<&'a Value>,
    path: &str,
    max: usize,
) -> Result<&'a Vec<Value>, ContentError> {
    let list = value
        .ok_or_else(|| shape(path, "is missing"))?
        .as_array()
        .ok_or_else(|| shape(path, "must be a list"))?;
    if list.len() > max {
        return Err(over(path, format!("more than {max} entries")));
    }
    Ok(list)
}

fn strings(value: Option<&Value>, path: &str, max: usize) -> Result<Vec<String>, ContentError> {
    list_of(value, path, max)?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let item = format!("{path}[{index}]");
            value
                .as_str()
                .ok_or_else(|| shape(&item, "must be a string"))
                .and_then(|text| bounded(text, &item))
        })
        .collect()
}
