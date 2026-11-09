//! Tones (FR-015, FR-017, FR-024, FR-025, T029): the only color a widget can ask for.

use design_tokens::{Appearance, HostColor, Icon, Rgb, Scale, Theme, Tone, UnknownTone, contrast, tone};

#[test]
fn parses_the_five_names_and_returns_any_other() {
    for (name, expected) in [
        ("default", Tone::Default),
        ("accent", Tone::Accent),
        ("ok", Tone::Ok),
        ("warning", Tone::Warning),
        ("error", Tone::Error),
    ] {
        assert_eq!(Tone::parse(name), Ok(expected));
    }
    assert_eq!(Tone::parse("host"), Err(UnknownTone("host".into())));
    assert_eq!(Tone::parse("Accent"), Err(UnknownTone("Accent".into())), "names are exact");
}

#[test]
fn each_tone_reads_its_source() {
    let blue = Appearance::DEFAULT;
    let violet = Appearance { widget: Scale::Violet, host: HostColor::Scale(Scale::Orange), ..blue };
    for theme in [Theme::Light, Theme::Dark] {
        // Default reads the gray family, never the host or widget color
        assert_eq!(tone(Tone::Default, &blue, theme), tone(Tone::Default, &violet, theme));
        // Accent reads the widget color
        assert_ne!(tone(Tone::Accent, &blue, theme).surface, tone(Tone::Accent, &violet, theme).surface);
        // Status tones are fixed: the person's picks do not move them
        for status in [Tone::Ok, Tone::Warning, Tone::Error] {
            assert_eq!(tone(status, &blue, theme), tone(status, &violet, theme), "{status:?}");
        }
    }
}

#[test]
fn a_status_is_never_color_alone() {
    for (status, icon) in [(Tone::Ok, Icon::Check), (Tone::Warning, Icon::Alert), (Tone::Error, Icon::Cross)] {
        let colors = tone(status, &Appearance::DEFAULT, Theme::Light);
        let (drawn, ink) = colors.icon.expect("a status tone has an icon");
        assert_eq!(drawn, icon);
        assert!(contrast(ink, colors.surface) >= 3.0, "{status:?} icon");
        assert!(!drawn.path().is_empty());
    }
    assert!(tone(Tone::Default, &Appearance::DEFAULT, Theme::Light).icon.is_none());
}

#[test]
fn tone_text_reads_on_its_surface() {
    for status in [Tone::Default, Tone::Accent, Tone::Ok, Tone::Warning, Tone::Error] {
        for theme in [Theme::Light, Theme::Dark] {
            let colors = tone(status, &Appearance::DEFAULT, theme);
            assert!(contrast(colors.text, colors.surface) >= 4.5, "{status:?} {theme:?}");
        }
    }
    assert_eq!(contrast(Rgb { r: 255, g: 255, b: 255 }, Rgb { r: 0, g: 0, b: 0 }), 21.0);
}
