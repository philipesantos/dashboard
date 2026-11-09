//! The contrast lint (constitution v5.0.0, Development Workflow; research R5): rules 10, 11 and
//! 13 of contracts/design-md.md. Every foreground role is measured on every background it is
//! drawn on, in every appearance the host accepts and in both themes.

use crate::check::{Appearance, Color, Design, Problem, SolidText};
use crate::color::{Rgb, Rgba, composite, contrast, cut2, hue_distance};
use crate::model::{RoleSource, Step};
use crate::radix::{GRAY_FAMILIES, Kind, Palette, Theme};
use crate::source::Source;
use std::collections::BTreeSet;

/// The contrast of `fg` drawn over the opaque `bg`, after compositing.
pub fn measure_over(fg: Rgba, bg: Rgb) -> f64 {
    contrast(composite(fg, bg), bg)
}

fn chromatic(palette: &Palette, design: &Design, name: &str) -> bool {
    palette
        .scales
        .get(name)
        .is_some_and(|scale| scale.kind(design.picks.neutral_below_chroma) == Kind::Chromatic)
}

/// Every appearance the host accepts, by `DESIGN.md`'s own picks: a gray family, a monochrome or
/// chromatic host, and a chromatic widget color at the rule's distance from the host color and the
/// status scales. The radius and theme pick do not change a color.
pub fn appearances<'a>(design: &'a Design, palette: &'a Palette) -> Vec<Appearance<'a>> {
    let colors: Vec<&str> = palette
        .scales
        .keys()
        .map(String::as_str)
        .filter(|name| !GRAY_FAMILIES.contains(name) && chromatic(palette, design, name))
        .collect();
    let hue = |name: &str| palette.scales[name].hue();
    let status: Vec<f64> = design.status.iter().map(|name| hue(name)).collect();
    let mut hosts: Vec<Option<&str>> = vec![None];
    hosts.extend(colors.iter().map(|name| Some(*name)));
    let mut out = Vec::new();
    for gray in &design.picks.gray {
        for host in &hosts {
            for widget in &colors {
                let far =
                    |other: f64| hue_distance(hue(widget), other) >= design.picks.hue_distance;
                if status.iter().all(|other| far(*other)) && host.is_none_or(|host| far(hue(host)))
                {
                    out.push(Appearance {
                        gray,
                        host: *host,
                        widget,
                    });
                }
            }
        }
    }
    out
}

fn line(source: &Source, key: &str, name: &str) -> usize {
    source
        .roles(key)
        .map_or(1, |(_, block)| block.line_of(name))
}

fn opaque(color: Color, under: Rgb) -> Rgb {
    match color {
        Color::Opaque(rgb) => rgb,
        Color::Translucent(rgba) => composite(rgba, under),
    }
}

fn describe(appearance: Appearance<'_>) -> String {
    format!(
        "gray family {}, host {}, widget {}",
        appearance.gray,
        appearance.host.unwrap_or("monochrome"),
        appearance.widget
    )
}

// Rule 10.
fn contrast_rule(design: &Design, palette: &Palette, source: &Source, problems: &mut Vec<Problem>) {
    let all = appearances(design, palette);
    let canvas = design.role("canvas");
    for spec in &design.roles {
        let Some(threshold) = spec.kind.and_then(|kind| kind.threshold()) else {
            continue;
        };
        for bg_name in &spec.on {
            let Some(bg) = design.role(bg_name) else {
                continue;
            };
            for theme in Theme::BOTH {
                let mut worst: Option<(f64, Appearance<'_>)> = None;
                for appearance in &all {
                    let resolve = |role| design.resolve(palette, role, theme, *appearance);
                    let (Ok(fg_color), Ok(bg_color)) = (resolve(&spec.role), resolve(&bg.role))
                    else {
                        continue;
                    };
                    let floor = canvas.and_then(|canvas| resolve(&canvas.role).ok()).map_or(
                        Rgb {
                            r: 255,
                            g: 255,
                            b: 255,
                        },
                        |c| {
                            opaque(
                                c,
                                Rgb {
                                    r: 255,
                                    g: 255,
                                    b: 255,
                                },
                            )
                        },
                    );
                    let under = opaque(bg_color, floor);
                    let ratio = contrast(opaque(fg_color, under), under);
                    if worst.is_none_or(|(least, _)| ratio < least) {
                        worst = Some((ratio, *appearance));
                    }
                }
                if let Some((ratio, appearance)) = worst
                    && ratio < threshold
                {
                    problems.push(Problem {
                        line: line(source, "roles", &spec.role.name),
                        path: format!("roles.{}", spec.role.name),
                        message: format!(
                            "on {bg_name} ({}, {} theme): {:.2}:1, under {threshold}:1, at {}",
                            spec.kind.expect("thresholded").name(),
                            theme.name(),
                            cut2(ratio),
                            describe(appearance)
                        ),
                    });
                }
            }
        }
    }
}

// Rule 11.
fn completeness_rule(design: &Design, source: &Source, problems: &mut Vec<Problem>) {
    let mut drawn: Vec<(String, String, String, usize)> = Vec::new();
    for tone in &design.tones {
        let at = line(source, "tones", tone.name.name());
        for fg in [Some(&tone.text), tone.border.as_ref(), tone.icon.as_ref()]
            .into_iter()
            .flatten()
        {
            drawn.push((
                fg.clone(),
                tone.surface.clone(),
                format!("tones.{}", tone.name.name()),
                at,
            ));
        }
    }
    for component in &design.components {
        let reference = |prop: &str| {
            component
                .props
                .iter()
                .find_map(|(name, value)| match value {
                    crate::check::PropValue::Reference { group, token }
                        if name == prop && group == "colors" =>
                    {
                        Some(token.strip_suffix("-dark").unwrap_or(token).to_owned())
                    }
                    _ => None,
                })
        };
        if let (Some(fg), Some(bg)) = (reference("textColor"), reference("backgroundColor")) {
            drawn.push((fg, bg, format!("components.{}", component.name), 1));
        }
    }
    if let Some(focus) = design.role("focus") {
        let mut backgrounds = BTreeSet::new();
        for component in design
            .components
            .iter()
            .filter(|c| c.name.starts_with("button") || c.name.starts_with("field"))
        {
            for (prop, value) in &component.props {
                if let crate::check::PropValue::Reference { group, token } = value
                    && prop == "backgroundColor"
                    && group == "colors"
                {
                    backgrounds.insert(token.clone());
                }
            }
        }
        for spec in &design.roles {
            if spec.role.source == RoleSource::Widget && matches!(spec.role.light, Step::Radix(9)) {
                backgrounds.insert(spec.role.name.clone());
            }
        }
        for bg in backgrounds {
            drawn.push((
                "focus".into(),
                bg,
                "roles.focus".into(),
                line(source, "roles", &focus.role.name),
            ));
        }
    }
    for (fg, bg, path, at) in drawn {
        match design.role(&fg) {
            Some(spec) if spec.kind.is_none() => problems.push(Problem {
                line: at,
                path,
                message: format!("draws {fg} as a foreground, but roles.{fg} has no kind"),
            }),
            Some(spec) if !spec.on.contains(&bg) => problems.push(Problem {
                line: at,
                path,
                message: format!("draws {fg} on {bg}, which roles.{fg}.on does not list, so its contrast is never checked"),
            }),
            _ => {}
        }
    }
}

// Rule 13.
fn solid_text_rule(
    design: &Design,
    palette: &Palette,
    source: &Source,
    problems: &mut Vec<Problem>,
) {
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let darks: Vec<Rgb> = design
        .picks
        .gray
        .iter()
        .map(|gray| palette.scales[gray].step(Theme::Light, 12))
        .collect();
    for (name, scale) in &palette.scales {
        if GRAY_FAMILIES.contains(&name.as_str()) {
            continue;
        }
        let at = line(source, "scales", name);
        let path = format!("scales.solid-text.{name}");
        let solids: Vec<Rgb> = Theme::BOTH
            .iter()
            .flat_map(|theme| [scale.step(*theme, 9), scale.step(*theme, 10)])
            .collect();
        let worst = |text: Rgb| {
            solids
                .iter()
                .map(|solid| contrast(text, *solid))
                .fold(f64::INFINITY, f64::min)
        };
        let white_worst = worst(white);
        let dark_worst = darks
            .iter()
            .map(|dark| worst(*dark))
            .fold(f64::INFINITY, f64::min);
        let message = match design.solid_text.get(name) {
            None => Some(
                "is missing: every scale declares the text its solid steps take (FR-009)"
                    .to_owned(),
            ),
            Some(SolidText::White) if white_worst < 4.5 => Some(format!(
                "white reaches only {:.2}:1 on steps 9-10",
                cut2(white_worst)
            )),
            Some(SolidText::Dark) if dark_worst < 4.5 => Some(format!(
                "dark text reaches only {:.2}:1 on steps 9-10",
                cut2(dark_worst)
            )),
            Some(SolidText::None) if white_worst >= 4.5 || dark_worst >= 4.5 => Some(format!(
                "is none, but {} text reaches 4.5:1 on steps 9-10",
                if white_worst >= 4.5 { "white" } else { "dark" }
            )),
            _ => None,
        };
        if let Some(message) = message {
            problems.push(Problem {
                line: at,
                path,
                message,
            });
        }
    }
}

/// Rules 10, 11 and 13.
pub fn lint(design: &Design, palette: &Palette, source: &Source) -> Vec<Problem> {
    let mut problems = Vec::new();
    completeness_rule(design, source, &mut problems);
    solid_text_rule(design, palette, source, &mut problems);
    contrast_rule(design, palette, source, &mut problems);
    problems
}
