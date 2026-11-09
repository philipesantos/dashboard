//! The color math every check rests on (specs/007-host-visual-design, T006): WCAG 2.2 contrast,
//! compositing a translucent color, OKLCH and hue distance.

use design_gen::color::{Rgb, Rgba, composite, contrast, cut2, hue_distance, oklch};

fn hex(text: &str) -> Rgb {
    Rgb::parse_hex(text).expect("a #rrggbb color")
}

#[test]
fn contrast_of_known_pairs() {
    assert_eq!(cut2(contrast(hex("#ffffff"), hex("#000000"))), 21.0);
    // The classic AA boundary: #767676 on white is 4.54, cut off, never rounded up
    assert_eq!(cut2(contrast(hex("#767676"), hex("#ffffff"))), 4.54);
    assert_eq!(contrast(hex("#3e63dd"), hex("#3e63dd")), 1.0);
    // Order does not matter
    assert_eq!(
        contrast(hex("#202020"), hex("#fcfcfc")),
        contrast(hex("#fcfcfc"), hex("#202020"))
    );
}

#[test]
fn cut_off_never_rounds_up() {
    assert_eq!(cut2(4.499_999), 4.49);
    assert_eq!(cut2(1.866), 1.86);
    assert_eq!(cut2(3.0), 3.0);
}

#[test]
fn compositing_over_an_opaque_color() {
    let half_black = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0.5,
    };
    let over_white = composite(half_black, hex("#ffffff"));
    for channel in [over_white.r, over_white.g, over_white.b] {
        assert!(
            channel.abs_diff(0x80) <= 1,
            "{over_white:?} is not #808080 within one unit"
        );
    }
    let opaque = Rgba {
        r: 12,
        g: 34,
        b: 56,
        a: 1.0,
    };
    assert_eq!(
        composite(opaque, hex("#ffffff")),
        Rgb {
            r: 12,
            g: 34,
            b: 56
        }
    );
    let clear = Rgba {
        r: 12,
        g: 34,
        b: 56,
        a: 0.0,
    };
    assert_eq!(composite(clear, hex("#abcdef")), hex("#abcdef"));
}

/// Radix 3.0.0's light step 9, against research R3's table (hue to 0.1°, chroma to 0.001).
#[test]
fn oklch_of_radix_step_9() {
    for (name, color, hue, chroma) in [
        ("blue", "#0090ff", 251.8, 0.193),
        ("red", "#e5484d", 23.0, 0.193),
        ("green", "#30a46c", 157.7, 0.133),
        ("amber", "#ffc53d", 84.1, 0.157),
    ] {
        let measured = oklch(hex(color));
        assert!(
            hue_distance(measured.h, hue) <= 0.1,
            "{name}: hue {} instead of {hue}",
            measured.h
        );
        assert!(
            (measured.c - chroma).abs() <= 0.001,
            "{name}: chroma {} instead of {chroma}",
            measured.c
        );
    }
    assert!(oklch(hex("#8d8d8d")).c < 0.001, "gray has no chroma");
}

#[test]
fn hue_distance_wraps_around() {
    assert_eq!(hue_distance(350.0, 10.0), 20.0);
    assert_eq!(hue_distance(10.0, 350.0), 20.0);
    assert_eq!(hue_distance(0.0, 180.0), 180.0);
    assert_eq!(hue_distance(90.0, 90.0), 0.0);
}

#[test]
fn hex_parsing_is_strict() {
    assert_eq!(
        hex("#0090FF"),
        Rgb {
            r: 0,
            g: 0x90,
            b: 0xff
        }
    );
    for bad in ["0090ff", "#09f", "#0090ff00", "#gg90ff", "", "#0090f"] {
        assert!(Rgb::parse_hex(bad).is_none(), "{bad:?} must be refused");
    }
    assert_eq!(hex("#0090ff").to_hex(), "#0090ff");
}
