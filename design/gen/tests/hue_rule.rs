//! SC-004: design-tokens' hue rule agrees, for every pair of picks, with a computation made here
//! from the vendored Radix data with an OKLCH implementation of its own, not design-gen's.

use crate::common::palette;
use design_tokens::{Appearance, HostColor, Refusal, Scale};

/// OKLCH hue and chroma, written again from Björn Ottosson's OKLab definition.
fn hue_chroma(hex: &str) -> (f64, f64) {
    let channel = |at: usize| {
        let c = f64::from(u8::from_str_radix(&hex[at..at + 2], 16).expect("hex")) / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (channel(1), channel(3), channel(5));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    let a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    let bb = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
    (bb.atan2(a).to_degrees().rem_euclid(360.0), a.hypot(bb))
}

fn distance(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

#[test]
fn every_pair_agrees_with_an_independent_computation() {
    let palette = palette();
    let measure = |scale: Scale| hue_chroma(&palette.scales[scale.name()].step(design_gen::radix::Theme::Light, 9).to_hex());
    let status: Vec<f64> = ["green", "amber", "red"]
        .iter()
        .map(|name| hue_chroma(&palette.scales[*name].step(design_gen::radix::Theme::Light, 9).to_hex()).0)
        .collect();
    let mut hosts = vec![HostColor::Monochrome];
    hosts.extend(Scale::ALL.into_iter().map(HostColor::Scale));
    let mut checked = 0;
    for host in hosts {
        for widget in Scale::ALL {
            let (widget_hue, widget_chroma) = measure(widget);
            let mut expected_ok = widget_chroma >= 0.08 && status.iter().all(|hue| distance(widget_hue, *hue) >= 30.0);
            if let HostColor::Scale(scale) = host {
                let (host_hue, host_chroma) = measure(scale);
                expected_ok &= host_chroma >= 0.08 && distance(widget_hue, host_hue) >= 30.0;
            }
            let verdict = Appearance { widget, host, ..Appearance::DEFAULT }.validate();
            assert_eq!(verdict.is_ok(), expected_ok, "{widget:?} with {host:?}: {verdict:?}");
            if let Err(Refusal::TooClose { degrees, .. }) = verdict {
                assert!(degrees < 30, "{widget:?} with {host:?} refused at {degrees}°");
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 26 * 25);
}
