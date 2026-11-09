//! Color math: sRGB, WCAG 2.2 relative luminance and contrast, compositing a translucent color over
//! an opaque one, OKLab/OKLCH and hue distance (specs/007-host-visual-design/research.md, R3, R5).

use std::fmt;

/// An opaque sRGB color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// An sRGB color with an alpha between 0 and 1, as Radix's `blackA` scale writes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Rgb {
    /// Parses `#rrggbb`, in either case, and nothing else.
    pub fn parse_hex(text: &str) -> Option<Rgb> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
        Some(Rgb {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
        })
    }

    /// `#rrggbb`, lowercase.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl Rgba {
    /// Parses `rgba(r, g, b, a)` with integer channels and an alpha from 0 to 1, as Radix writes
    /// it, and nothing else.
    pub fn parse_rgba(text: &str) -> Option<Rgba> {
        let inner = text.strip_prefix("rgba(")?.strip_suffix(')')?;
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        let [r, g, b, a] = parts.as_slice() else {
            return None;
        };
        let alpha: f64 = a.parse().ok()?;
        if !(0.0..=1.0).contains(&alpha) {
            return None;
        }
        Some(Rgba {
            r: r.parse().ok()?,
            g: g.parse().ok()?,
            b: b.parse().ok()?,
            a: alpha,
        })
    }
}

fn linear(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 2.2 relative luminance.
pub fn relative_luminance(color: Rgb) -> f64 {
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

/// WCAG 2.2 contrast ratio, from 1 to 21, exact: thresholds compare against this value, never a
/// rounded one.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// A ratio cut off to two decimals for messages, never rounded up, so 4.499 shows as 4.49.
pub fn cut2(ratio: f64) -> f64 {
    (ratio * 100.0 + 1e-9).floor() / 100.0
}

/// `top` drawn over the opaque `bottom`, in sRGB as a browser or GPUI blends it, each channel
/// rounded to the nearest integer.
pub fn composite(top: Rgba, bottom: Rgb) -> Rgb {
    let mix = |over: u8, under: u8| {
        (f64::from(over) * top.a + f64::from(under) * (1.0 - top.a)).round() as u8
    };
    Rgb {
        r: mix(top.r, bottom.r),
        g: mix(top.g, bottom.g),
        b: mix(top.b, bottom.b),
    }
}

/// A color in OKLCH: lightness 0–1, chroma, hue in degrees 0–360.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f64,
    pub c: f64,
    pub h: f64,
}

/// OKLCH of an sRGB color, through OKLab (Björn Ottosson's matrices).
pub fn oklch(color: Rgb) -> Oklch {
    let (r, g, b) = (linear(color.r), linear(color.g), linear(color.b));
    let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
    let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
    let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    let lightness = 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s;
    let a = 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s;
    let b = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s;
    Oklch {
        l: lightness,
        c: a.hypot(b),
        h: b.atan2(a).to_degrees().rem_euclid(360.0),
    }
}

/// The shorter way round the hue circle between two hues, 0–180°.
pub fn hue_distance(a: f64, b: f64) -> f64 {
    let difference = (a - b).rem_euclid(360.0);
    difference.min(360.0 - difference)
}
