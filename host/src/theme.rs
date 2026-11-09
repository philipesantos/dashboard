//! The tokens `DESIGN.md` generates, as GPUI values (FR-006). Nothing else in the host names a
//! color, a type size, a radius or a line: every screen reads them from a [`Theme`].

use design_tokens::typography::Level;
use design_tokens::{Appearance, Radius, Rgb, ThemePick, role_color, typography};
use gpui::{FontWeight, Hsla, Pixels, Rgba, WindowAppearance, px};

/// One color for each role the host draws with, under an appearance in a theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub canvas: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub text_secondary: Hsla,
    pub divider: Hsla,
    pub border_control: Hsla,
    pub neutral_soft: Hsla,
    pub accent_soft: Hsla,
    pub accent_strong: Hsla,
    pub ok_icon: Hsla,
    pub warning_icon: Hsla,
    pub error_icon: Hsla,
    pub chrome: Hsla,
    pub chrome_text: Hsla,
    pub panel: Hsla,
    pub dialog: Hsla,
    pub scrim: Hsla,
    pub focus: Hsla,
    pub ink: Hsla,
    pub on_ink: Hsla,
    /// The person's corner radius, in pixels.
    pub radius: Pixels,
}

pub(crate) fn color(rgb: Rgb) -> Hsla {
    Rgba {
        r: f32::from(rgb.r) / 255.0,
        g: f32::from(rgb.g) / 255.0,
        b: f32::from(rgb.b) / 255.0,
        a: 1.0,
    }
    .into()
}

/// The theme the person's pick resolves to, given the window's appearance for `System`.
pub fn resolve_theme(pick: ThemePick, window: WindowAppearance) -> design_tokens::Theme {
    match pick {
        ThemePick::Light => design_tokens::Theme::Light,
        ThemePick::Dark => design_tokens::Theme::Dark,
        ThemePick::System => match window {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => design_tokens::Theme::Dark,
            WindowAppearance::Light | WindowAppearance::VibrantLight => design_tokens::Theme::Light,
        },
    }
}

impl Theme {
    pub fn new(appearance: &Appearance, theme: design_tokens::Theme) -> Theme {
        let role = |name: &str| {
            color(
                role_color(name, appearance, theme)
                    .unwrap_or_else(|| panic!("DESIGN.md defines the role {name}")),
            )
        };
        let chrome = host_surfaces::chrome(appearance, theme);
        let dialog = host_surfaces::dialog(appearance, theme);
        let ink = host_surfaces::ink(appearance, theme);
        let mut scrim = color(dialog.scrim);
        scrim.a = dialog.scrim_opacity as f32;
        Theme {
            dark: theme == design_tokens::Theme::Dark,
            canvas: role("canvas"),
            surface: role("surface"),
            text: role("text"),
            text_secondary: role("text-secondary"),
            divider: role("divider"),
            border_control: role("border-control"),
            neutral_soft: role("neutral-soft"),
            accent_soft: role("accent-soft"),
            accent_strong: role("accent-strong"),
            ok_icon: role("ok-icon"),
            warning_icon: role("warning-icon"),
            error_icon: role("error-icon"),
            chrome: color(chrome.surface),
            chrome_text: color(chrome.text),
            panel: color(host_surfaces::panel_attachment(appearance, theme).surface),
            dialog: color(dialog.surface),
            scrim,
            focus: color(host_surfaces::focus(appearance, theme).ring),
            ink: color(ink.fill),
            on_ink: color(ink.text),
            radius: px(appearance.radius.px()),
        }
    }

    /// The radius of the monogram: always a circle (DESIGN.md, Shapes).
    pub fn full_radius() -> Pixels {
        px(Radius::Full.px())
    }
}

/// A type level of `DESIGN.md`, as GPUI's text settings. GPUI 1.25.3 has no letter spacing, so
/// `label-mono`'s 0.06 em is not applied; its capitals are (research R4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Type {
    pub family: &'static str,
    pub size: Pixels,
    pub weight: FontWeight,
    pub line_height: Pixels,
    pub capitals: bool,
}

fn level(level: Level, capitals: bool) -> Type {
    Type {
        family: level.family,
        size: px(level.size),
        weight: FontWeight(f32::from(level.weight)),
        line_height: px(level.line_height),
        capitals,
    }
}

pub fn caption() -> Type {
    level(typography::CAPTION, false)
}
pub fn label_sm() -> Type {
    level(typography::LABEL_SM, false)
}
pub fn body_md() -> Type {
    level(typography::BODY_MD, false)
}
pub fn label_md() -> Type {
    level(typography::LABEL_MD, false)
}
pub fn label_mono() -> Type {
    level(typography::LABEL_MONO, true)
}
pub fn title_sm() -> Type {
    level(typography::TITLE_SM, false)
}
pub fn title_md() -> Type {
    level(typography::TITLE_MD, false)
}
