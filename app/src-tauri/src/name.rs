//! The name a widget's frame draws (specs/009-declarative-widget-ui/research.md, R6; criterion
//! HS-02). The manifest refuses control characters and names over 60 characters when the plugin
//! loads; here the host removes what could change how the name reads, and keeps it short.
//!
//! Part of the security core (constitution, Principle IX).

/// The longest name a frame draws, ellipsis included.
pub const DRAWN_NAME_CHARS: usize = 40;

/// The name to draw for a plugin: `name` without control or format characters (bidirectional
/// overrides and isolates, zero-width characters, tags), whitespace collapsed, cut at
/// [`DRAWN_NAME_CHARS`] with an ellipsis. When nothing is left, the last part of the plugin's
/// `id` (`local/showcase` → `showcase`). The page draws it as text, never as markup.
pub fn drawn_name(name: Option<&str>, id: &str) -> String {
    let kept: String = name
        .unwrap_or_default()
        .chars()
        .filter(|&c| !c.is_control() && !is_format(c))
        .collect();
    let collapsed = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return id.rsplit('/').next().unwrap_or(id).to_owned();
    }
    if collapsed.chars().count() <= DRAWN_NAME_CHARS {
        return collapsed;
    }
    let cut: String = collapsed.chars().take(DRAWN_NAME_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Unicode's format characters (general category Cf), as of Unicode 16.
fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061C}'
            | '\u{06DD}'
            | '\u{070F}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08E2}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{13430}'..='\u{1343F}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0001}'
            | '\u{E0020}'..='\u{E007F}'
    )
}
