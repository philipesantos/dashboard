//! The person's appearance picks (FR-010-FR-013, T022): the default, the 30° hue rule against the
//! host color and the fixed status scales, and the neutral scales refused as picks.

use design_tokens::{Appearance, GrayFamily, HostColor, Other, Radius, Refusal, Scale, ThemePick};

fn with(widget: Scale, host: HostColor) -> Appearance {
    Appearance { widget, host, ..Appearance::DEFAULT }
}

#[test]
fn the_default_is_neutral_and_valid() {
    let default = Appearance::DEFAULT;
    assert_eq!(default.gray, GrayFamily::Gray);
    assert_eq!(default.host, HostColor::Monochrome);
    assert_eq!(default.widget, Scale::Blue);
    assert_eq!(default.radius, Radius::Medium);
    assert_eq!(default.theme, ThemePick::System);
    assert_eq!(default.validate(), Ok(()));
}

#[test]
fn a_widget_color_near_the_host_color_is_refused() {
    assert_eq!(
        with(Scale::Blue, HostColor::Scale(Scale::Indigo)).validate(),
        Err(Refusal::TooClose { widget: Scale::Blue, other: Other::Host(Scale::Indigo), degrees: 15 })
    );
    assert_eq!(
        with(Scale::Blue, HostColor::Scale(Scale::Blue)).validate(),
        Err(Refusal::TooClose { widget: Scale::Blue, other: Other::Host(Scale::Blue), degrees: 0 })
    );
    assert_eq!(with(Scale::Blue, HostColor::Scale(Scale::Purple)).validate(), Ok(()));
}

#[test]
fn a_widget_color_near_a_status_is_refused() {
    assert!(matches!(
        with(Scale::Grass, HostColor::Monochrome).validate(),
        Err(Refusal::TooClose { other: Other::Ok, .. })
    ));
    assert!(matches!(
        with(Scale::Yellow, HostColor::Monochrome).validate(),
        Err(Refusal::TooClose { other: Other::Warning, .. })
    ));
    assert!(matches!(
        with(Scale::Tomato, HostColor::Monochrome).validate(),
        Err(Refusal::TooClose { other: Other::Error, .. })
    ));
}

#[test]
fn neutral_scales_are_never_picks() {
    for neutral in [Scale::Bronze, Scale::Gold, Scale::Brown] {
        assert_eq!(
            with(neutral, HostColor::Monochrome).validate(),
            Err(Refusal::NotChromatic { scale: neutral }),
            "{neutral:?} as the widget color"
        );
        assert_eq!(
            with(Scale::Blue, HostColor::Scale(neutral)).validate(),
            Err(Refusal::NotChromatic { scale: neutral }),
            "{neutral:?} as the host color"
        );
    }
}

/// With a monochrome host, exactly these ten widget colors are accepted (research R3).
#[test]
fn the_widget_colors_a_monochrome_host_accepts() {
    let accepted: Vec<Scale> =
        Scale::ALL.into_iter().filter(|scale| with(*scale, HostColor::Monochrome).validate().is_ok()).collect();
    assert_eq!(
        accepted,
        [
            Scale::Plum,
            Scale::Purple,
            Scale::Violet,
            Scale::Iris,
            Scale::Indigo,
            Scale::Blue,
            Scale::Cyan,
            Scale::Pink,
            Scale::Sky,
            Scale::Lime,
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
    );
}

#[test]
fn radius_picks_match_design_md() {
    let names: Vec<&str> = Radius::ALL.iter().map(|radius| radius.name()).collect();
    assert_eq!(names, ["none", "small", "medium", "large", "full"]);
    assert_eq!(Radius::Medium.px(), 6.0);
    assert!(Radius::Full.px() > 1000.0);
}
