//! The contrast lint (FR-008, T025): it covers every appearance the host accepts, in both themes,
//! and measures translucent colors after compositing.

use crate::common::{design_md, palette};
use design_tokens::{Appearance, GrayFamily, HostColor, Scale};

#[test]
fn covers_exactly_the_appearances_design_tokens_accepts() {
    let palette = palette();
    let design = design_gen::check_text(&design_md(), &palette).expect("valid");
    let linted = design_gen::lint::appearances(&design, &palette);
    let mut hosts = vec![HostColor::Monochrome];
    hosts.extend(Scale::ALL.into_iter().map(HostColor::Scale));
    let mut accepted = std::collections::BTreeSet::new();
    for gray in GrayFamily::ALL {
        for host in &hosts {
            for widget in Scale::ALL {
                let appearance = Appearance {
                    gray,
                    host: *host,
                    widget,
                    ..Appearance::DEFAULT
                };
                if appearance.validate().is_ok() {
                    let host = match host {
                        HostColor::Monochrome => None,
                        HostColor::Scale(scale) => Some(scale.name().to_owned()),
                    };
                    accepted.insert((gray.name().to_owned(), host, widget.name().to_owned()));
                }
            }
        }
    }
    let linted: std::collections::BTreeSet<(String, Option<String>, String)> = linted
        .iter()
        .map(|a| {
            (
                a.gray.to_owned(),
                a.host.map(str::to_owned),
                a.widget.to_owned(),
            )
        })
        .collect();
    assert_eq!(linted, accepted);
    assert!(
        accepted.len() > 1000 && accepted.len() <= 1380,
        "{} appearances",
        accepted.len()
    );
}

#[test]
fn translucent_colors_are_composited_before_measuring() {
    use design_gen::color::{Rgb, Rgba, composite, contrast};
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let half = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0.5,
    };
    let measured = design_gen::lint::measure_over(half, white);
    assert_eq!(measured, contrast(composite(half, white), white));
    assert!(measured > 1.0 && measured < 21.0);
}
