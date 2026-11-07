//! Widget design tokens: the only colors, sizes and spacing widget content is drawn with.

use gpui_kit::Hsla;
use serde::Deserialize;

/// Typography shared by host and widgets (font sizes in pixels).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Typography {
    /// Font families in order of preference; the host uses the first one installed.
    pub font_families: Vec<String>,
    pub size_small: f32,
    pub size_body: f32,
    pub size_heading: f32,
    pub line_height: f32,
}

/// Spacing scale in pixels.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub radius: f32,
}

/// The widget palette, as `#rrggbb` strings in the tokens file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetPalette {
    pub surface: Color,
    pub text: Color,
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub positive: Color,
    pub warning: Color,
    pub danger: Color,
    pub row_alt: Color,
    pub focus_ring: Color,
}

impl WidgetPalette {
    /// Every color, for the host's check that none of them can pass for a host color.
    pub fn all(&self) -> [(&'static str, Color); 11] {
        [
            ("surface", self.surface),
            ("text", self.text),
            ("muted", self.muted),
            ("border", self.border),
            ("accent", self.accent),
            ("accent_text", self.accent_text),
            ("positive", self.positive),
            ("warning", self.warning),
            ("danger", self.danger),
            ("row_alt", self.row_alt),
            ("focus_ring", self.focus_ring),
        ]
    }
}

/// What widget content is drawn with: the palette of the current theme, typography, spacing.
#[derive(Debug, Clone)]
pub struct WidgetTheme {
    pub palette: WidgetPalette,
    pub typography: Typography,
    pub spacing: Spacing,
}

/// An sRGB color read from `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    pub fn hsla(self) -> Hsla {
        gpui_kit::rgb(self.0).into()
    }

    pub fn channels(self) -> [f32; 3] {
        let c = self.0;
        [
            ((c >> 16) & 0xff) as f32,
            ((c >> 8) & 0xff) as f32,
            (c & 0xff) as f32,
        ]
    }

    /// Euclidean distance in RGB, 0 to ~441.
    pub fn distance(self, other: Color) -> f32 {
        let (a, b) = (self.channels(), other.channels());
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    /// WCAG relative luminance.
    pub fn luminance(self) -> f32 {
        let lin = |v: f32| {
            let v = v / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b] = self.channels();
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    /// WCAG contrast ratio, 1 to 21.
    pub fn contrast(self, other: Color) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Color, D::Error> {
        let text = String::deserialize(deserializer)?;
        let hex = text
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .ok_or_else(|| serde::de::Error::custom(format!("{text:?} is not #rrggbb")))?;
        u32::from_str_radix(hex, 16)
            .map(Color)
            .map_err(|_| serde::de::Error::custom(format!("{text:?} is not #rrggbb")))
    }
}
