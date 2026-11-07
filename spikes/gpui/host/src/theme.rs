//! Design tokens (design/tokens.json): the widget palette, which widget-kit draws content with,
//! and the host palette, which only the host's own chrome (frame, toolbar, dialogs) uses. The
//! widget kit never receives the host palette or the visual secret.

use std::collections::BTreeMap;

use serde::Deserialize;
use widget_kit::tokens::{Color, Spacing, Typography, WidgetPalette, WidgetTheme};

const TOKENS: &str = include_str!("../../design/tokens.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostPalette {
    pub canvas: Color,
    pub toolbar: Color,
    pub frame: Color,
    pub frame_border: Color,
    pub frame_text: Color,
    pub frame_focus: Color,
    pub dialog: Color,
    pub dialog_border: Color,
    pub dialog_text: Color,
    pub scrim: Color,
    pub host_accent: Color,
    pub host_accent_text: Color,
    pub drop_target: Color,
}

impl HostPalette {
    /// The colors reserved to the host: what marks a frame or a dialog as the host's. No widget
    /// color may come close to them (see the tests).
    pub fn reserved(&self) -> [(&'static str, Color); 5] {
        [
            ("frame_border", self.frame_border),
            ("frame_focus", self.frame_focus),
            ("dialog_border", self.dialog_border),
            ("host_accent", self.host_accent),
            ("drop_target", self.drop_target),
        ]
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeTokens {
    widget: WidgetPalette,
    host: HostPalette,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tokens {
    typography: Typography,
    spacing: Spacing,
    secret_choices: BTreeMap<String, Color>,
    light: ModeTokens,
    dark: ModeTokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, Deserialize)]
pub enum Mode {
    #[default]
    System,
    Light,
    Dark,
}

/// The theme the host draws with for one appearance.
#[derive(Debug, Clone)]
pub struct Theme {
    pub dark: bool,
    pub widget: WidgetTheme,
    pub host: HostPalette,
    pub typography: Typography,
    pub spacing: Spacing,
}

pub fn theme(dark: bool) -> Theme {
    let tokens = tokens();
    let mode = if dark { tokens.dark } else { tokens.light };
    Theme {
        dark,
        widget: WidgetTheme {
            palette: mode.widget,
            typography: tokens.typography.clone(),
            spacing: tokens.spacing.clone(),
        },
        host: mode.host,
        typography: tokens.typography,
        spacing: tokens.spacing,
    }
}

/// The colors a person can choose as their visual secret, by name.
pub fn secret_choices() -> BTreeMap<String, Color> {
    tokens().secret_choices
}

fn tokens() -> Tokens {
    serde_json::from_str(TOKENS).expect("design/tokens.json is valid (tested)")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RGB distance under which two colors are taken as the same by eye.
    const DISTINCT: f32 = 60.0;

    #[test]
    fn tokens_parse() {
        let _ = tokens();
        assert!(secret_choices().len() >= 4);
    }

    /// A widget cannot imitate the host: no widget color comes near a reserved host color or
    /// a secret choice, in either theme.
    #[test]
    fn widgets_cannot_reach_host_colors() {
        for dark in [false, true] {
            let theme = theme(dark);
            let mut reserved: Vec<(String, Color)> = theme
                .host
                .reserved()
                .iter()
                .map(|(n, c)| (n.to_string(), *c))
                .collect();
            reserved.extend(
                secret_choices()
                    .into_iter()
                    .map(|(n, c)| (format!("secret {n}"), c)),
            );
            for (widget_name, widget_color) in theme.widget.palette.all() {
                for (host_name, host_color) in &reserved {
                    let distance = widget_color.distance(*host_color);
                    assert!(
                        distance >= DISTINCT,
                        "dark={dark}: widget {widget_name} {widget_color:?} is {distance:.0} from host {host_name} {host_color:?}"
                    );
                }
            }
        }
    }

    /// Text meets WCAG AA (4.5:1) on the surface it is drawn on.
    #[test]
    fn text_contrast_is_aa() {
        for dark in [false, true] {
            let t = theme(dark);
            let (w, h) = (&t.widget.palette, &t.host);
            let pairs = [
                ("widget text", w.text, w.surface),
                ("widget muted", w.muted, w.surface),
                ("widget accent text", w.accent_text, w.accent),
                ("widget danger", w.danger, w.surface),
                ("widget table header", w.text, w.row_alt),
                ("frame text", h.frame_text, h.frame),
                ("dialog text", h.dialog_text, h.dialog),
                ("host accent text", h.host_accent_text, h.host_accent),
                ("toolbar text", h.frame_text, h.toolbar),
            ];
            for (name, fg, bg) in pairs {
                let ratio = fg.contrast(bg);
                assert!(
                    ratio >= 4.5,
                    "dark={dark}: {name} contrast {ratio:.2} < 4.5"
                );
            }
        }
    }
}
