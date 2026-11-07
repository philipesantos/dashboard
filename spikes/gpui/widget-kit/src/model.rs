//! Widget content: the closed format a plugin's `render` output must follow.
//!
//! Unknown block types, unknown fields and tones outside the widget palette are refused, so a
//! plugin cannot name a password field, a host color or anything the kit does not draw. Text is
//! cleaned of control and bidirectional formatting characters, which could reorder what the
//! person reads (a file name shown backwards, for example).

use serde::Deserialize;

/// Limits on one widget's content. The runtime already caps the JSON at 64 KiB.
pub const MAX_BLOCKS: usize = 64;
pub const MAX_TEXT_CHARS: usize = 2_000;
pub const MAX_LIST_ITEMS: usize = 200;
pub const MAX_TABLE_COLUMNS: usize = 8;
pub const MAX_TABLE_ROWS: usize = 200;
pub const MAX_BUTTONS: usize = 8;
pub const MAX_ACTION_CHARS: usize = 64;

/// One widget's content, as validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Block {
    Text {
        text: String,
        #[serde(default)]
        style: TextStyle,
        #[serde(default)]
        tone: Tone,
    },
    List {
        items: Vec<String>,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// A button only asks the host for `action`; the host decides what, if anything, happens.
    Button {
        label: String,
        action: String,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextStyle {
    Heading,
    #[default]
    Body,
}

/// The widget palette's tones. The host's colors are not among them, by construction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tone {
    #[default]
    Default,
    Muted,
    Accent,
    Positive,
    Warning,
    Danger,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    blocks: Vec<Block>,
}

/// Content of the first widget format, `{"title": …, "text": …}`, which hello-widget returns.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TitleText {
    title: Option<String>,
    text: Option<String>,
}

/// Why content was refused, for the frame's error line and the maintainer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentError(pub String);

impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Content {
    /// Reads and validates a plugin's render output.
    pub fn parse(value: &serde_json::Value) -> Result<Content, ContentError> {
        let blocks = if value.get("blocks").is_some() {
            serde_json::from_value::<Document>(value.clone())
                .map_err(|error| ContentError(format!("widget content: {error}")))?
                .blocks
        } else {
            let old = serde_json::from_value::<TitleText>(value.clone())
                .map_err(|error| ContentError(format!("widget content: {error}")))?;
            let mut blocks = Vec::new();
            if let Some(title) = old.title {
                blocks.push(Block::Text {
                    text: title,
                    style: TextStyle::Heading,
                    tone: Tone::Default,
                });
            }
            if let Some(text) = old.text {
                blocks.push(Block::Text {
                    text,
                    style: TextStyle::Body,
                    tone: Tone::Default,
                });
            }
            blocks
        };
        let mut content = Content { blocks };
        content.check_and_clean()?;
        Ok(content)
    }

    fn check_and_clean(&mut self) -> Result<(), ContentError> {
        if self.blocks.len() > MAX_BLOCKS {
            return Err(over(format!(
                "{} blocks, at most {MAX_BLOCKS}",
                self.blocks.len()
            )));
        }
        let buttons = self
            .blocks
            .iter()
            .filter(|b| matches!(b, Block::Button { .. }))
            .count();
        if buttons > MAX_BUTTONS {
            return Err(over(format!("{buttons} buttons, at most {MAX_BUTTONS}")));
        }
        for (index, block) in self.blocks.iter_mut().enumerate() {
            let at = |what: &str| format!("blocks[{index}].{what}");
            match block {
                Block::Text { text, .. } => clean(text, &at("text"))?,
                Block::List { items } => {
                    if items.len() > MAX_LIST_ITEMS {
                        return Err(over(format!(
                            "{}: {} items, at most {MAX_LIST_ITEMS}",
                            at("items"),
                            items.len()
                        )));
                    }
                    for item in items.iter_mut() {
                        clean(item, &at("items"))?;
                    }
                }
                Block::Table { columns, rows } => {
                    if columns.is_empty() || columns.len() > MAX_TABLE_COLUMNS {
                        return Err(over(format!(
                            "{}: {} columns, from 1 to {MAX_TABLE_COLUMNS}",
                            at("columns"),
                            columns.len()
                        )));
                    }
                    if rows.len() > MAX_TABLE_ROWS {
                        return Err(over(format!(
                            "{}: {} rows, at most {MAX_TABLE_ROWS}",
                            at("rows"),
                            rows.len()
                        )));
                    }
                    for column in columns.iter_mut() {
                        clean(column, &at("columns"))?;
                    }
                    let width = columns.len();
                    for (r, row) in rows.iter_mut().enumerate() {
                        if row.len() != width {
                            return Err(ContentError(format!(
                                "{}[{r}] has {} cells; the table has {width} columns",
                                at("rows"),
                                row.len()
                            )));
                        }
                        for cell in row.iter_mut() {
                            clean(cell, &at("rows"))?;
                        }
                    }
                }
                Block::Button { label, action } => {
                    clean(label, &at("label"))?;
                    if label.trim().is_empty() {
                        return Err(ContentError(format!("{} is empty", at("label"))));
                    }
                    let valid = !action.is_empty()
                        && action.len() <= MAX_ACTION_CHARS
                        && action
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                    if !valid {
                        return Err(ContentError(format!(
                            "{}: an action is 1 to {MAX_ACTION_CHARS} ASCII letters, digits, '-' or '_'",
                            at("action")
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

fn over(detail: String) -> ContentError {
    ContentError(format!("widget content over the limit: {detail}"))
}

/// Removes control and bidirectional formatting characters, then checks the length.
fn clean(text: &mut String, path: &str) -> Result<(), ContentError> {
    if text.chars().any(is_hidden) {
        *text = text.chars().filter(|c| !is_hidden(*c)).collect();
    }
    let chars = text.chars().count();
    if chars > MAX_TEXT_CHARS {
        return Err(over(format!(
            "{path}: {chars} characters, at most {MAX_TEXT_CHARS}"
        )));
    }
    Ok(())
}

/// Characters that change how text reads without being seen: C0/C1 controls (newline and tab
/// are kept), bidirectional embeddings, overrides and isolates, and the zero-width marks.
fn is_hidden(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t')
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{061C}' | '\u{FEFF}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(value: serde_json::Value) -> Result<Content, ContentError> {
        Content::parse(&value)
    }

    #[test]
    fn the_four_blocks_parse() {
        let content = parse(json!({"blocks": [
            {"type": "text", "text": "a", "style": "heading", "tone": "accent"},
            {"type": "list", "items": ["x", "y"]},
            {"type": "table", "columns": ["c"], "rows": [["1"]]},
            {"type": "button", "label": "Go", "action": "go"}
        ]}))
        .unwrap();
        assert_eq!(content.blocks.len(), 4);
    }

    #[test]
    fn hello_widget_output_parses() {
        let content = parse(json!({"title": "Hello", "text": "Hello, world!"})).unwrap();
        assert_eq!(content.blocks.len(), 2);
    }

    #[test]
    fn a_password_field_is_refused() {
        let error = parse(json!({"blocks": [{"type": "password-field", "label": "Password"}]}))
            .unwrap_err();
        assert!(error.0.contains("password-field"), "{error}");
    }

    #[test]
    fn host_tones_cannot_be_named() {
        for tone in ["host-secret", "host-accent", "frame", "dialog", "secret"] {
            let error = parse(json!({"blocks": [{"type": "text", "text": "x", "tone": tone}]}))
                .unwrap_err();
            assert!(error.0.contains("unknown variant"), "{tone}: {error}");
        }
    }

    #[test]
    fn unknown_fields_are_refused() {
        // No style escape hatch: a color, a position or an overlay cannot be smuggled in
        for field in [
            json!({"color": "#ff0000"}),
            json!({"position": "absolute"}),
            json!({"html": "<b>"}),
        ] {
            let mut block = json!({"type": "text", "text": "x"});
            block
                .as_object_mut()
                .unwrap()
                .extend(field.as_object().unwrap().clone());
            assert!(parse(json!({"blocks": [block]})).is_err());
        }
        assert!(parse(json!({"blocks": [], "window": {"title": "x"}})).is_err());
    }

    #[test]
    fn bidirectional_controls_are_removed() {
        let content = parse(
            json!({"blocks": [{"type": "text", "text": "a\u{202E}txt.exe\u{202C}b\u{200B}"}]}),
        )
        .unwrap();
        assert_eq!(
            content.blocks[0],
            Block::Text {
                text: "atxt.exeb".into(),
                style: TextStyle::Body,
                tone: Tone::Default
            }
        );
    }

    #[test]
    fn limits_hold() {
        let long = "x".repeat(MAX_TEXT_CHARS + 1);
        assert!(parse(json!({"blocks": [{"type": "text", "text": long}]})).is_err());
        let blocks: Vec<_> = (0..=MAX_BLOCKS)
            .map(|_| json!({"type": "text", "text": "x"}))
            .collect();
        assert!(parse(json!({"blocks": blocks})).is_err());
        assert!(
            parse(json!({"blocks": [{"type": "table", "columns": ["a", "b"], "rows": [["1"]]}]}))
                .is_err()
        );
        assert!(
            parse(json!({"blocks": [{"type": "button", "label": "x", "action": "rm -rf"}]}))
                .is_err()
        );
    }
}
