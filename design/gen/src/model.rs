//! The entities of specs/007-host-visual-design/data-model.md, as the checker builds them from
//! `DESIGN.md` and the generator writes them out. Construction only; the rules are in `check`.

use crate::color::Rgb;

/// Which pick or fixed scale a role reads (data-model.md, Role).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RoleSource {
    /// The person's gray family.
    Gray,
    /// The person's widget color.
    Widget,
    /// The person's host color; the gray family when the host is monochrome.
    Host,
    /// The fixed status scales: green, amber and red (FR-024).
    Ok,
    Warning,
    Error,
    /// `blackA`, Radix's overlay scale, for the scrim and shadows (research R2).
    Overlay,
}

impl RoleSource {
    pub const ALL: [RoleSource; 7] = [
        RoleSource::Gray,
        RoleSource::Widget,
        RoleSource::Host,
        RoleSource::Ok,
        RoleSource::Warning,
        RoleSource::Error,
        RoleSource::Overlay,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RoleSource::Gray => "gray",
            RoleSource::Widget => "widget",
            RoleSource::Host => "host",
            RoleSource::Ok => "ok",
            RoleSource::Warning => "warning",
            RoleSource::Error => "error",
            RoleSource::Overlay => "overlay",
        }
    }

    pub fn parse(name: &str) -> Option<RoleSource> {
        RoleSource::ALL
            .into_iter()
            .find(|source| source.name() == name)
    }
}

/// The step of a role in one theme: a Radix step, white or black (constitution v5.0.0), or the
/// text color the scale declares for its solid steps (FR-009).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    Radix(u8),
    White,
    Black,
    SolidText,
}

/// A color role of the project layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    /// kebab-case, unique.
    pub name: String,
    pub source: RoleSource,
    pub light: Step,
    pub dark: Step,
    /// Required (FR-001).
    pub why: String,
    /// True routes it to the `host-surfaces` crate (research R6).
    pub host_surface: bool,
}

/// What a pairing is checked as (research R5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PairingKind {
    /// WCAG 1.4.3, 4.5:1.
    Text,
    /// WCAG 1.4.3 for large text, 3:1.
    LargeText,
    /// WCAG 1.4.11, 3:1: control boundaries, focus indicators, meaningful icons.
    NonText,
    /// Not checked: only under WCAG's exemption for decoration, with a reason.
    Decorative,
    /// Not checked: only under WCAG's exemption for inactive components, with a reason.
    Inactive,
}

impl PairingKind {
    pub fn parse(name: &str) -> Option<PairingKind> {
        Some(match name {
            "text" => PairingKind::Text,
            "large-text" => PairingKind::LargeText,
            "non-text" => PairingKind::NonText,
            "decorative" => PairingKind::Decorative,
            "inactive" => PairingKind::Inactive,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            PairingKind::Text => "text",
            PairingKind::LargeText => "large-text",
            PairingKind::NonText => "non-text",
            PairingKind::Decorative => "decorative",
            PairingKind::Inactive => "inactive",
        }
    }

    /// The minimum ratio, or `None` for the two exempt kinds.
    pub fn threshold(self) -> Option<f64> {
        match self {
            PairingKind::Text => Some(4.5),
            PairingKind::LargeText | PairingKind::NonText => Some(3.0),
            PairingKind::Decorative | PairingKind::Inactive => None,
        }
    }
}

/// A foreground role over a background role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    pub fg: String,
    pub bg: String,
    pub kind: PairingKind,
    /// Required for `decorative` and `inactive`.
    pub why: Option<String>,
}

/// The five tones a widget can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ToneName {
    Default,
    Accent,
    Ok,
    Warning,
    Error,
}

impl ToneName {
    pub const ALL: [ToneName; 5] = [
        ToneName::Default,
        ToneName::Accent,
        ToneName::Ok,
        ToneName::Warning,
        ToneName::Error,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ToneName::Default => "default",
            ToneName::Accent => "accent",
            ToneName::Ok => "ok",
            ToneName::Warning => "warning",
            ToneName::Error => "error",
        }
    }

    pub fn is_status(self) -> bool {
        matches!(self, ToneName::Ok | ToneName::Warning | ToneName::Error)
    }
}

/// A tone, as roles (data-model.md, Tone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tone {
    pub name: ToneName,
    pub text: String,
    pub surface: String,
    pub border: Option<String>,
    pub icon: Option<String>,
    /// Required for ok, warning and error (FR-025): the name of an icon in `icons`.
    pub status_icon: Option<String>,
}

/// The value of a generated token.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenValue {
    Color(Rgb),
    /// In pixels.
    Dimension(f64),
    Number(f64),
    Typography {
        family: String,
        size: f64,
        weight: u16,
        line_height: f64,
        letter_spacing: Option<String>,
        feature: Option<String>,
    },
    Shadow(String),
    Text(String),
}

/// Anything generated (data-model.md, Token).
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// Dotted, unique across both layers, such as `typography.body-md`.
    pub path: String,
    pub value: TokenValue,
    /// Required (FR-001).
    pub why: String,
    pub host_surface: bool,
}

/// One `HS-NN` entry of docs/design/host-surfaces.md (research R10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Criterion {
    /// `HS-` and two digits; unique, never reused.
    pub id: String,
    pub title: String,
    pub observe: String,
    pub expect: String,
    /// The constitution rules and the threat rows it answers, rows by their exact first cell.
    pub answers: String,
    pub tested_by: String,
    pub line: usize,
}
