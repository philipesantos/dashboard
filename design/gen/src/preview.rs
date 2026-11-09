//! The SVG preview (specs/007-host-visual-design/research.md, R8): static files a reviewer opens
//! in a browser. No script, no reference outside the preview directory; Inter is embedded as
//! WOFF2 so the text looks the same everywhere. Everything is drawn from the design's tokens.

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

    fn icon(&mut self, x: f64, y: f64, path: &str, fill: &str) {
        let _ = writeln!(
            self.body,
            r#"<path transform="translate({x} {y})" fill-rule="evenodd" fill="{fill}" d="{}"/>"#,
            xml(path)
        );
    }

    fn finish(self, title: &str, font: Option<&str>, background: &str) -> String {
        let style = font.map_or(String::new(), |font| {
            format!(
                "<style>@font-face{{font-family:\"Inter\";src:url(data:font/woff2;base64,{font}) format(\"woff2\");font-weight:100 900}}text{{font-family:\"Inter\",sans-serif}}</style>\n"
            )
        });
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

fn window(canvas: &mut Canvas, paint: &Paint<'_>, x0: f64, dialog: bool) {
    let (w, h) = (720.0, 520.0);
    let radius = paint.radius();
    canvas.rect((x0, 40.0, w, h), 8.0, &paint.color("canvas"), "");
    // The chrome: the host's own bar
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

    // A widget frame with its label and monogram
    let (fx, fy, fw, fh) = (x0 + 16.0, 96.0, 400.0, 300.0);
    // The frame is outlined by the divider: one step off the canvas is too faint alone
    let frame_edge = format!(
        "stroke=\"{}\" stroke-width=\"1\" {}",
        paint.color("divider"),
        paint.shadow("frame")
    );
    canvas.rect(
        (fx, fy, fw, fh),
        radius,
        &paint.color("surface"),
        &frame_edge,
    );
    canvas.rect(
        (fx + 12.0, fy + 12.0, 20.0, 20.0),
        3.0,
        &paint.color("neutral-soft"),
        "",
    );
    let (small, small_weight) = paint.size("label-sm");
    canvas.text(
        fx + 17.5,
        fy + 26.5,
        small,
        small_weight,
        &paint.color("text"),
        "T",
    );
    canvas.text(
        fx + 40.0,
        fy + 26.5,
        small,
        small_weight,
        &paint.color("text-secondary"),
        "Tasks",
    );
    let (body, body_weight) = paint.size("body-md");
    canvas.text(
        fx + 12.0,
        fy + 60.0,
        body,
        body_weight,
        &paint.color("text"),
        "Review the visual system",
    );
    canvas.text(
        fx + 12.0,
        fy + 82.0,
        body,
        body_weight,
        &paint.color("accent-text"),
        "Open the preview artifact",
    );

    // Status badges: icon and words, never color alone
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
            12.0,
            &paint.color(&format!("{tone}-soft")),
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
        canvas.text(
            bx + 28.0,
            by + 16.5,
            small,
            small_weight,
            &paint.color("text"),
            word,
        );
    }

    // Two declared actions, as the host draws them
    canvas.rect(
        (fx + 12.0, fy + 144.0, 96.0, 28.0),
        radius,
        &paint.color("neutral-soft"),
        "",
    );
    canvas.text(
        fx + 30.0,
        fy + 162.5,
        label,
        weight,
        &paint.color("text"),
        "Snooze",
    );
    canvas.rect(
        (fx + 116.0, fy + 144.0, 96.0, 28.0),
        radius,
        &paint.color("accent-soft"),
        "",
    );
    canvas.text(
        fx + 136.0,
        fy + 162.5,
        label,
        weight,
        &paint.color("accent-strong"),
        "Complete",
    );

    // A field the host draws for a plugin, with the host's note
    let field = format!(
        "stroke=\"{}\" stroke-width=\"1\"",
        paint.color("border-control")
    );
    canvas.rect(
        (fx + 12.0, fy + 192.0, fw - 24.0, 28.0),
        radius,
        &paint.color("surface"),
        &field,
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
    // The focus ring, the host's own style
    let focus = format!(
        "fill=\"none\" stroke=\"{}\" stroke-width=\"2\"",
        paint.color("focus")
    );
    let _ = writeln!(
        canvas.body,
        r#"<rect x="{}" y="{}" width="100" height="32" rx="{}" {focus}/>"#,
        fx + 10.0,
        fy + 142.0,
        radius + 2.0
    );

    // The panel the frame opens, attached to it
    let (px, py) = (fx + fw + 8.0, fy);
    canvas.rect(
        (px, py, 280.0, 200.0),
        radius,
        &paint.color("panel-surface"),
        &paint.shadow("panel"),
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
    canvas.text(
        px + 16.0,
        py + 54.0,
        small,
        small_weight,
        &paint.color("text-secondary"),
        "Settings of the Tasks widget",
    );

    if dialog {
        // A host dialog: the scrim covers the whole window, chrome included
        canvas.rect((x0, 40.0, w, h), 8.0, &paint.color("scrim"), "");
        let (dx, dy) = (x0 + 120.0, 140.0);
        canvas.rect(
            (dx, dy, 480.0, 260.0),
            10.0,
            &paint.color("dialog-surface"),
            &paint.shadow("dialog"),
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
        canvas.rect(
            (dx + 264.0, dy + 208.0, 96.0, 28.0),
            radius,
            &paint.color("neutral-soft"),
            "",
        );
        canvas.text(
            dx + 288.0,
            dy + 226.5,
            label,
            weight,
            &paint.color("text"),
            "Cancel",
        );
        canvas.rect(
            (dx + 368.0, dy + 208.0, 88.0, 28.0),
            radius,
            &paint.color("accent-soft"),
            "",
        );
        canvas.text(
            dx + 390.0,
            dy + 226.5,
            label,
            weight,
            &paint.color("accent-strong"),
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

/// The preview files, as names and SVG text.
pub fn render(
    design: &Design,
    palette: &Palette,
    design_dir: &Path,
) -> Result<Vec<(String, String)>, String> {
    let font_path = design_dir
        .join("fonts")
        .join("inter")
        .join("InterVariable.woff2");
    let font = base64(
        &std::fs::read(&font_path).map_err(|error| format!("{}: {error}", font_path.display()))?,
    );
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
