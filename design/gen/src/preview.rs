//! The SVG preview (specs/007-host-visual-design/research.md, R8): static files a reviewer opens
//! in a browser. No script, no reference outside the preview directory; the host's fonts, Barlow
//! and IBM Plex Mono, are embedded as WOFF2 so the text looks the same everywhere. Everything is
//! drawn from the design's tokens.

use crate::check::{Appearance, Color, Design};
use crate::color::{Rgb, composite};
use crate::radix::{GRAY_FAMILIES, Kind, Palette, SCALES, Theme};
use std::fmt::Write as _;
use std::path::Path;

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(TABLE[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Text escaped for XML.
fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

struct Canvas {
    body: String,
    width: u32,
    height: u32,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Canvas {
        Canvas {
            body: String::new(),
            width,
            height,
        }
    }

    fn rect(&mut self, (x, y, w, h): (f64, f64, f64, f64), radius: f64, fill: &str, extra: &str) {
        let _ = writeln!(
            self.body,
            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{radius}" fill="{fill}" {extra}/>"#
        );
    }

    fn text(&mut self, x: f64, y: f64, size: f64, weight: u16, fill: &str, content: &str) {
        let _ = writeln!(
            self.body,
            r#"<text x="{x}" y="{y}" font-size="{size}" font-weight="{weight}" fill="{fill}">{}</text>"#,
            xml(content)
        );
    }

    /// Text in the `label-mono` style: IBM Plex Mono, in capitals, spaced as the level says.
    fn mono(&mut self, x: f64, y: f64, level: (f64, u16, f64), fill: &str, content: &str) {
        let (size, weight, spacing) = level;
        let _ = writeln!(
            self.body,
            r#"<text x="{x}" y="{y}" font-size="{size}" font-weight="{weight}" letter-spacing="{spacing}" font-family="IBM Plex Mono, monospace" fill="{fill}">{}</text>"#,
            xml(&content.to_uppercase())
        );
    }

    fn icon(&mut self, x: f64, y: f64, path: &str, fill: &str) {
        let _ = writeln!(
            self.body,
            r#"<path transform="translate({x} {y})" fill-rule="evenodd" fill="{fill}" d="{}"/>"#,
            xml(path)
        );
    }

    fn finish(self, title: &str, fonts: Option<&str>, background: &str) -> String {
        let style = fonts.map_or(String::new(), |css| format!("<style>{css}</style>\n"));
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n<title>{}</title>\n{style}<rect width=\"{w}\" height=\"{h}\" fill=\"{background}\"/>\n{}</svg>\n",
            xml(title),
            self.body,
            w = self.width,
            h = self.height
        )
    }
}

/// Resolves roles of the design at one appearance and theme, as hex for SVG.
struct Paint<'a> {
    design: &'a Design,
    palette: &'a Palette,
    theme: Theme,
    appearance: Appearance<'a>,
}

impl Paint<'_> {
    fn color(&self, role: &str) -> String {
        let spec = self
            .design
            .role(role)
            .unwrap_or_else(|| panic!("DESIGN.md has a role {role}"));
        match self
            .design
            .resolve(self.palette, &spec.role, self.theme, self.appearance)
            .expect("checked")
        {
            Color::Opaque(rgb) => rgb.to_hex(),
            Color::Translucent(rgba) => {
                format!("rgba({},{},{},{})", rgba.r, rgba.g, rgba.b, rgba.a)
            }
        }
    }

    fn shadow(&self, level: &str) -> String {
        self.design
            .elevation
            .iter()
            .find(|candidate| candidate.name == level)
            .and_then(|level| level.shadow)
            .map_or(String::new(), |shadow| {
                let alpha = self.palette.black_a[usize::from(shadow.black_a) - 1].a;
                format!(
                    "style=\"filter:drop-shadow({}px {}px {}px rgba(0,0,0,{alpha}))\"",
                    shadow.x,
                    shadow.y,
                    shadow.blur / 2.0
                )
            })
    }

    fn size(&self, level: &str) -> (f64, u16) {
        self.design
            .typography
            .iter()
            .find(|(name, _)| name == level)
            .map(|(_, level)| (level.size, level.weight))
            .unwrap_or((14.0, 400))
    }

    /// The `label-mono` level: size, weight and letter spacing in pixels.
    fn mono(&self) -> (f64, u16, f64) {
        self.design
            .typography
            .iter()
            .find(|(name, _)| name == "label-mono")
            .map_or((11.0, 500, 0.66), |(_, level)| {
                (
                    level.size,
                    level.weight,
                    level.size * level.letter_spacing_em.unwrap_or(0.0),
                )
            })
    }

    fn radius(&self) -> f64 {
        let pick = &self.design.picks.default.radius;
        self.design
            .rounded
            .iter()
            .find(|(name, _)| name == pick)
            .map_or(6.0, |(_, value)| *value)
    }

    fn icon(&self, name: &str) -> &str {
        self.design
            .icons
            .iter()
            .find(|(icon, _)| icon == name)
            .map_or("", |(_, path)| path)
    }
}

fn palette_svg(design: &Design, palette: &Palette, theme: Theme, font: &str) -> String {
    let swatch = 40.0;
    let left = 200.0;
    let mut canvas = Canvas::new(
        (left + swatch * 12.0 + 340.0) as u32,
        (80.0 + 46.0 * SCALES.len() as f64) as u32,
    );
    let (background, ink) = match theme {
        Theme::Light => ("#ffffff", "#202020"),
        Theme::Dark => ("#111111", "#eeeeee"),
    };
    canvas.text(
        24.0,
        36.0,
        20.0,
        600,
        ink,
        &format!(
            "Radix 3.0.0, {} theme: steps 1-12, and what each scale can be picked as",
            theme.name()
        ),
    );
    let status: Vec<(&str, f64)> = design
        .status
        .iter()
        .map(|name| (name.as_str(), palette.scales[name].hue()))
        .collect();
    for (row, name) in SCALES.iter().enumerate() {
        let y = 64.0 + 46.0 * row as f64;
        let scale = &palette.scales[*name];
        canvas.text(24.0, y + 26.0, 14.0, 500, ink, name);
        for step in 1..=12u8 {
            let x = left + swatch * f64::from(step - 1);
            canvas.rect(
                (x, y, swatch - 2.0, 38.0),
                3.0,
                &scale.step(theme, step).to_hex(),
                "",
            );
        }
        let verdict = match scale.kind(design.picks.neutral_below_chroma) {
            Kind::GrayFamily => "gray family".to_owned(),
            Kind::Neutral => format!("neutral (chroma {:.3}): not a pick", scale.chroma()),
            Kind::Chromatic => {
                let close: Vec<String> = status
                    .iter()
                    .filter(|(_, hue)| {
                        crate::color::hue_distance(scale.hue(), *hue) < design.picks.hue_distance
                    })
                    .map(|(name, hue)| {
                        format!(
                            "{name} ({:.0}°)",
                            crate::color::hue_distance(scale.hue(), *hue)
                        )
                    })
                    .collect();
                if close.is_empty() {
                    format!("hue {:.1}°: widget color", scale.hue())
                } else {
                    format!(
                        "hue {:.1}°: host only, near {}",
                        scale.hue(),
                        close.join(", ")
                    )
                }
            }
        };
        canvas.text(
            left + swatch * 12.0 + 16.0,
            y + 26.0,
            13.0,
            400,
            ink,
            &verdict,
        );
    }
    let _ = GRAY_FAMILIES;
    canvas.finish(
        &format!("Radix palette, {}", theme.name()),
        Some(font),
        background,
    )
}

/// A stroke attribute in a role's color.
fn stroke(paint: &Paint<'_>, role: &str) -> String {
    format!("stroke=\"{}\" stroke-width=\"1\"", paint.color(role))
}

/// A button as the host draws it: a fill, a step-9 boundary and a monospaced label.
fn button(
    canvas: &mut Canvas,
    paint: &Paint<'_>,
    (x, y, w): (f64, f64, f64),
    (fill, ink): (&str, &str),
    label: &str,
) {
    let edge = stroke(paint, "border-control");
    canvas.rect(
        (x + 0.5, y + 0.5, w - 1.0, 27.0),
        paint.radius(),
        &paint.color(fill),
        &edge,
    );
    canvas.mono(x + 12.0, y + 18.0, paint.mono(), &paint.color(ink), label);
}

fn window(canvas: &mut Canvas, paint: &Paint<'_>, x0: f64, dialog: bool) {
    let (w, h) = (720.0, 520.0);
    let radius = paint.radius();
    let mono = paint.mono();
    canvas.rect((x0, 40.0, w, h), 8.0, &paint.color("canvas"), "");
    // The chrome: the host's own bar, closed by the divider
    canvas.rect((x0, 40.0, w, 40.0), 0.0, &paint.color("chrome-surface"), "");
    let (label, weight) = paint.size("label-md");
    canvas.text(
        x0 + 16.0,
        65.0,
        label,
        weight,
        &paint.color("text"),
        "Dashboard",
    );
    canvas.rect((x0, 79.0, w, 1.0), 0.0, &paint.color("divider"), "");

    // A widget frame: one sheet, outlined by the divider, with no shadow. Edges sit on half
    // pixels so a 1 px line covers one pixel.
    let (fx, fy, fw, fh) = (x0 + 16.0, 96.0, 400.0, 300.0);
    let frame_edge = format!("{} {}", stroke(paint, "divider"), paint.shadow("frame"));
    canvas.rect(
        (fx + 0.5, fy + 0.5, fw - 1.0, fh - 1.0),
        radius,
        &paint.color("surface"),
        &frame_edge,
    );
    // The header the host draws: the monogram on the host's ink, the label, a header action
    let ink = paint.color("host-strong");
    let on_ink = paint.color("on-host-strong");
    let _ = writeln!(
        canvas.body,
        r#"<circle cx="{}" cy="{}" r="10" fill="{ink}"/>"#,
        fx + 22.0,
        fy + 18.0
    );
    let (small, small_weight) = paint.size("label-sm");
    canvas.text(fx + 18.5, fy + 22.5, small, small_weight, &on_ink, "T");
    canvas.mono(
        fx + 40.0,
        fy + 22.0,
        mono,
        &paint.color("text-secondary"),
        "Tasks",
    );
    canvas.rect((fx + fw - 72.0, fy + 6.0, 62.0, 24.0), radius, &ink, "");
    canvas.mono(fx + fw - 62.0, fy + 22.0, mono, &on_ink, "+ Add");
    canvas.rect(
        (fx + 1.0, fy + 36.0, fw - 2.0, 1.0),
        0.0,
        &paint.color("divider"),
        "",
    );

    let (body, body_weight) = paint.size("body-md");
    canvas.text(
        fx + 12.0,
        fy + 62.0,
        body,
        body_weight,
        &paint.color("text"),
        "Review the visual system",
    );
    canvas.text(
        fx + 12.0,
        fy + 84.0,
        body,
        body_weight,
        &paint.color("accent-text"),
        "Open the preview artifact",
    );

    // Status badges: the icon in its color, the words in text color, never color alone
    for (index, (tone, word)) in [
        ("ok", "Done"),
        ("warning", "Due today"),
        ("error", "Failed"),
    ]
    .iter()
    .enumerate()
    {
        let bx = fx + 12.0 + 120.0 * index as f64;
        let by = fy + 104.0;
        canvas.rect(
            (bx, by, 110.0, 24.0),
            radius,
            &paint.color("neutral-soft"),
            "",
        );
        let icon = match *tone {
            "ok" => "check",
            "warning" => "alert",
            _ => "cross",
        };
        canvas.icon(
            bx + 6.0,
            by + 4.0,
            paint.icon(icon),
            &paint.color(&format!("{tone}-icon")),
        );
        canvas.mono(bx + 28.0, by + 16.0, mono, &paint.color("text"), word);
    }

    // Two declared actions, as the host draws them
    button(
        canvas,
        paint,
        (fx + 12.0, fy + 144.0, 96.0),
        ("neutral-soft", "text"),
        "Snooze",
    );
    button(
        canvas,
        paint,
        (fx + 116.0, fy + 144.0, 104.0),
        ("accent-soft", "accent-strong"),
        "Complete",
    );

    // A field the host draws for a plugin, with the host's note
    canvas.rect(
        (fx + 12.5, fy + 192.5, fw - 25.0, 27.0),
        radius,
        &paint.color("surface"),
        &stroke(paint, "border-control"),
    );
    canvas.text(
        fx + 20.0,
        fy + 210.5,
        body,
        body_weight,
        &paint.color("text"),
        "Groceries",
    );
    let (caption, caption_weight) = paint.size("caption");
    canvas.text(
        fx + 12.0,
        fy + 238.0,
        caption,
        caption_weight,
        &paint.color("text-secondary"),
        "Text typed here goes to the Tasks widget.",
    );
    canvas.text(
        fx + 12.0,
        fy + 254.0,
        caption,
        caption_weight,
        &paint.color("text-secondary"),
        "It is not for passwords, tokens or codes.",
    );
    // The focus ring, the host's own style: 2 px, 2 px outside the control
    let focus = format!(
        "fill=\"none\" stroke=\"{}\" stroke-width=\"2\"",
        paint.color("focus")
    );
    let _ = writeln!(
        canvas.body,
        r#"<rect x="{}" y="{}" width="102" height="34" rx="{}" {focus}/>"#,
        fx + 9.0,
        fy + 141.0,
        radius + 2.0
    );

    // The panel the frame opens, attached to it
    let (px, py) = (fx + fw + 8.0, fy);
    let panel_edge = format!("{} {}", stroke(paint, "divider"), paint.shadow("panel"));
    canvas.rect(
        (px + 0.5, py + 0.5, 279.0, 199.0),
        radius,
        &paint.color("panel-surface"),
        &panel_edge,
    );
    let (title, title_weight) = paint.size("title-sm");
    canvas.text(
        px + 16.0,
        py + 32.0,
        title,
        title_weight,
        &paint.color("text"),
        "Tasks settings",
    );
    canvas.mono(
        px + 16.0,
        py + 54.0,
        mono,
        &paint.color("text-secondary"),
        "Settings of the Tasks widget",
    );

    if dialog {
        // A host dialog: the scrim covers the whole window, chrome included
        canvas.rect((x0, 40.0, w, h), 8.0, &paint.color("scrim"), "");
        let (dx, dy) = (x0 + 120.0, 140.0);
        let dialog_edge = format!("{} {}", stroke(paint, "divider"), paint.shadow("dialog"));
        canvas.rect(
            (dx + 0.5, dy + 0.5, 479.0, 259.0),
            radius,
            &paint.color("dialog-surface"),
            &dialog_edge,
        );
        let (heading, heading_weight) = paint.size("title-md");
        canvas.text(
            dx + 24.0,
            dy + 44.0,
            heading,
            heading_weight,
            &paint.color("text"),
            "Install Tasks?",
        );
        canvas.text(
            dx + 24.0,
            dy + 76.0,
            body,
            body_weight,
            &paint.color("text-secondary"),
            "From github.com/example/tasks, release 1.2.0",
        );
        canvas.text(
            dx + 24.0,
            dy + 104.0,
            body,
            body_weight,
            &paint.color("text"),
            "It asks to keep up to 1 MiB of data on this computer.",
        );
        button(
            canvas,
            paint,
            (dx + 256.0, dy + 208.0, 96.0),
            ("neutral-soft", "text"),
            "Cancel",
        );
        button(
            canvas,
            paint,
            (dx + 360.0, dy + 208.0, 96.0),
            ("accent-soft", "accent-strong"),
            "Install",
        );
    }
}

fn default_svg(design: &Design, palette: &Palette, theme: Theme, font: &str) -> String {
    let paint = Paint {
        design,
        palette,
        theme,
        appearance: design.default_appearance(),
    };
    let mut canvas = Canvas::new(1488, 580);
    let ink = paint.color("text");
    canvas.text(16.0, 26.0, 16.0, 600, &ink, &format!("Default appearance, {} theme: the dashboard, and the same window with a host dialog open", theme.name()));
    window(&mut canvas, &paint, 16.0, false);
    window(&mut canvas, &paint, 752.0, true);
    let background = match design.resolve(
        palette,
        &design.role("canvas").expect("canvas").role,
        theme,
        paint.appearance,
    ) {
        Ok(Color::Opaque(rgb)) => composite(
            crate::color::Rgba {
                r: rgb.r,
                g: rgb.g,
                b: rgb.b,
                a: 1.0,
            },
            Rgb { r: 0, g: 0, b: 0 },
        )
        .to_hex(),
        _ => "#ffffff".to_owned(),
    };
    canvas.finish(
        &format!("Default appearance, {}", theme.name()),
        Some(font),
        &background,
    )
}

fn index_svg() -> String {
    let mut canvas = Canvas::new(640, 240);
    canvas.text(24.0, 40.0, 20.0, 600, "#202020", "DESIGN.md preview");
    for (row, (file, what)) in [
        (
            "palette-light.svg",
            "Every Radix scale, light theme, and what it can be picked as",
        ),
        ("palette-dark.svg", "Every Radix scale, dark theme"),
        (
            "default-light.svg",
            "The default appearance, light: chrome, frame, panel, dialog",
        ),
        ("default-dark.svg", "The default appearance, dark"),
    ]
    .iter()
    .enumerate()
    {
        let y = 84.0 + 36.0 * row as f64;
        let _ = writeln!(
            canvas.body,
            r##"<a href="{file}"><text x="24" y="{y}" font-size="15" fill="#0d74ce" font-family="sans-serif">{}</text></a>"##,
            xml(what)
        );
    }
    canvas.finish("DESIGN.md preview", None, "#ffffff")
}

/// The host's fonts the preview embeds: family, weight and file under `design/fonts/`.
const FONTS: [(&str, u16, &str); 5] = [
    ("Barlow", 400, "barlow/Barlow-Regular.woff2"),
    ("Barlow", 500, "barlow/Barlow-Medium.woff2"),
    ("Barlow", 600, "barlow/Barlow-SemiBold.woff2"),
    ("IBM Plex Mono", 400, "plex-mono/IBMPlexMono-Regular.woff2"),
    ("IBM Plex Mono", 500, "plex-mono/IBMPlexMono-Medium.woff2"),
];

/// The preview files, as names and SVG text.
pub fn render(
    design: &Design,
    palette: &Palette,
    design_dir: &Path,
) -> Result<Vec<(String, String)>, String> {
    let mut font = String::new();
    for (family, weight, file) in FONTS {
        let path = design_dir.join("fonts").join(file);
        let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let _ = write!(
            font,
            "@font-face{{font-family:\"{family}\";src:url(data:font/woff2;base64,{}) format(\"woff2\");font-weight:{weight}}}",
            base64(&bytes)
        );
    }
    font.push_str("text{font-family:\"Barlow\",sans-serif}");
    Ok(vec![
        ("index.svg".to_owned(), index_svg()),
        (
            "palette-light.svg".to_owned(),
            palette_svg(design, palette, Theme::Light, &font),
        ),
        (
            "palette-dark.svg".to_owned(),
            palette_svg(design, palette, Theme::Dark, &font),
        ),
        (
            "default-light.svg".to_owned(),
            default_svg(design, palette, Theme::Light, &font),
        ),
        (
            "default-dark.svg".to_owned(),
            default_svg(design, palette, Theme::Dark, &font),
        ),
    ])
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_known_values() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
    }
}
