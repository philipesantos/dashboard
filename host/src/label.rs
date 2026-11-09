//! A plugin's name made safe for its frame (docs/design/host-surfaces.md, HS-06 and HS-07;
//! research R8). The name a manifest declares is the plugin's own text: it reaches a screen only
//! as a [`PluginText`], which only this module makes, drawn as plain text under the host's words.

use crate::text::{HostText, Msg};
use unicode_properties::{GeneralCategory, GeneralCategoryGroup, UnicodeGeneralCategory};
use unicode_segmentation::UnicodeSegmentation;

/// The most grapheme clusters a label keeps before its ellipsis (HS-06).
pub const MAX_LABEL_GRAPHEMES: usize = 40;

/// The monogram when a name has no letter or digit (HS-07): a fixed glyph of the host's font.
pub const MONOGRAM_FALLBACK: &str = "•";

/// A plugin's text, cleaned by this module. Only [`label`] makes one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PluginText(String);

impl PluginText {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a frame shows as its label: the plugin's cleaned name, or the host's "Unnamed widget"
/// when nothing is left.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Label {
    Plugin(PluginText),
    Unnamed(HostText),
}

impl Label {
    pub fn as_str(&self) -> &str {
        match self {
            Label::Plugin(text) => text.as_str(),
            Label::Unnamed(text) => text.as_str(),
        }
    }
}

/// Removes every control, format, line separator and paragraph separator character (Unicode Cc,
/// Cf, Zl, Zp): bidirectional overrides and zero-width characters among them.
fn clean(name: &str) -> String {
    name.chars()
        .filter(|c| {
            !matches!(
                c.general_category(),
                GeneralCategory::Control
                    | GeneralCategory::Format
                    | GeneralCategory::LineSeparator
                    | GeneralCategory::ParagraphSeparator
            )
        })
        .collect()
}

/// The frame's label for a plugin named `name` (HS-06): cleaned, capped at
/// [`MAX_LABEL_GRAPHEMES`] grapheme clusters with an ellipsis, or "Unnamed widget".
pub fn label(name: &str) -> Label {
    let cleaned = clean(name);
    let visible = cleaned.trim();
    if visible.is_empty() {
        return Label::Unnamed(Msg::UnnamedWidget.text());
    }
    let graphemes: Vec<&str> = visible.graphemes(true).collect();
    let text = if graphemes.len() > MAX_LABEL_GRAPHEMES {
        let mut cut: String = graphemes[..MAX_LABEL_GRAPHEMES].concat();
        cut.push('…');
        cut
    } else {
        visible.to_owned()
    };
    Label::Plugin(PluginText(text))
}

/// The frame's monogram for a plugin named `name` (HS-07): the first letter or digit of the
/// cleaned name, upper-cased where its script has case, or [`MONOGRAM_FALLBACK`].
pub fn monogram(name: &str) -> String {
    clean(name)
        .chars()
        .find(|c| {
            c.general_category_group() == GeneralCategoryGroup::Letter
                || c.general_category() == GeneralCategory::DecimalNumber
        })
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| MONOGRAM_FALLBACK.to_owned())
}
