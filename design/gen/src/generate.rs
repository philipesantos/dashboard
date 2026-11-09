//! Generation (specs/007-host-visual-design/research.md, R7): the Rust tokens of `design-tokens`
//! and `host-surfaces`, split by the host-surface marks, and the W3C Design Tokens export. The
//! output is deterministic, LF-only and committed; the drift test compares it with the files.

use crate::check::{Design, SolidText};
use crate::model::{RoleSource, Step};
use crate::radix::{GRAY_FAMILIES, Palette, SCALES, Theme};
use std::fmt::Write as _;
use std::path::Path;

pub const TOKENS_RS: &str = "design/tokens/src/generated.rs";
pub const HOST_SURFACES_RS: &str = "design/host-surfaces/src/generated.rs";
pub const DTCG_JSON: &str = "design/tokens.dtcg.json";

const HEADER: &str = "// Generated from DESIGN.md by `cargo xtask design`. Do not edit.\n";

/// The generated files, as repository-relative paths and their text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    pub files: Vec<(String, String)>,
}

/// `body-md` → `BODY_MD`; `2xl` → `XL2`, since a Rust name cannot start with a digit.
fn constant(name: &str) -> String {
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    let rest = &name[digits.len()..];
    format!("{}{digits}", rest.to_ascii_uppercase().replace('-', "_"))
}

/// `body-md` → `BodyMd`.
fn variant(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// A Rust string literal.
fn literal(text: &str) -> String {
    format!("{text:?}")
}

/// A one-line doc comment, with the token path first so the drift test can name it.
fn doc(out: &mut String, indent: &str, path: &str, why: &str) {
    let _ = writeln!(out, "{indent}/// {path}: {}", why.replace('\n', " "));
}

fn float(value: f64) -> String {
    let text = format!("{value}");
    if text.contains('.') || text.contains('e') {
        text
    } else {
        format!("{text}.0")
    }
}

fn source_variant(source: RoleSource) -> &'static str {
    match source {
        RoleSource::Gray => "Source::Gray",
        RoleSource::Widget => "Source::Widget",
        RoleSource::Host => "Source::Host",
        RoleSource::Ok => "Source::Ok",
        RoleSource::Warning => "Source::Warning",
        RoleSource::Error => "Source::Error",
        RoleSource::Overlay => "Source::Overlay",
    }
}

fn step_variant(step: Step) -> String {
    match step {
        Step::Radix(n) => format!("StepRef::Radix({n})"),
        Step::White => "StepRef::White".into(),
        Step::Black => "StepRef::Black".into(),
        Step::SolidText => "StepRef::SolidText".into(),
    }
}

const ROLE_TYPES: &str = "\
/// Which pick or fixed scale a role reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Gray,
    Widget,
    Host,
    Ok,
    Warning,
    Error,
    Overlay,
}

/// The step a role takes in one theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StepRef {
    Radix(u8),
    White,
    Black,
    SolidText,
}

/// A color role: where it reads its color in each theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RoleDef {
    pub(crate) name: &'static str,
    pub(crate) source: Source,
    pub(crate) light: StepRef,
    pub(crate) dark: StepRef,
}
";

fn roles(out: &mut String, design: &Design, host_surface: bool) {
    let _ = writeln!(out, "pub(crate) const ROLES: &[RoleDef] = &[");
    for spec in design
        .roles
        .iter()
        .filter(|spec| spec.role.host_surface == host_surface)
    {
        let role = &spec.role;
        // A plain comment: doc comments are not allowed on array elements
        let _ = writeln!(
            out,
            "    // roles.{}: {}",
            role.name,
            role.why.replace('\n', " ")
        );
        let _ = writeln!(
            out,
            "    RoleDef {{ name: {}, source: {}, light: {}, dark: {} }},",
            literal(&role.name),
            source_variant(role.source),
            step_variant(role.light),
            step_variant(role.dark)
        );
    }
    let _ = writeln!(out, "];");
}

fn rgb_table(out: &mut String, name: &str, rows: &[(&str, [crate::color::Rgb; 12])]) {
    let _ = writeln!(
        out,
        "pub(crate) const {name}: [[[u8; 3]; 12]; {}] = [",
        rows.len()
    );
    for (scale, steps) in rows {
        let cells: Vec<String> = steps
            .iter()
            .map(|c| format!("[{}, {}, {}]", c.r, c.g, c.b))
            .collect();
        let _ = writeln!(out, "    // {scale}\n    [{}],", cells.join(", "));
    }
    let _ = writeln!(out, "];");
}

fn elevation(out: &mut String, design: &Design, host_surface: bool) {
    for level in design
        .elevation
        .iter()
        .filter(|level| level.host_surface == host_surface)
    {
        doc(
            out,
            "    ",
            &format!("elevation.{}", level.name),
            &level.why,
        );
        match level.shadow {
            None => {
                let _ = writeln!(
                    out,
                    "    pub const {}: Option<Shadow> = None;",
                    constant(&level.name)
                );
            }
            Some(shadow) => {
                let _ = writeln!(
                    out,
                    "    pub const {}: Option<Shadow> = Some(Shadow {{ x: {}, y: {}, blur: {}, black_a: {} }});",
                    constant(&level.name),
                    float(shadow.x),
                    float(shadow.y),
                    float(shadow.blur),
                    shadow.black_a
                );
            }
        }
    }
}

const SHADOW_TYPE: &str = "\
    /// A shadow: offsets and blur in pixels, its color a step of Radix's blackA.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Shadow {
        pub x: f32,
        pub y: f32,
        pub blur: f32,
        pub black_a: u8,
    }
";

fn tokens_rs(design: &Design, palette: &Palette) -> String {
    let mut out = String::from(HEADER);
    let chromatic: Vec<&str> = SCALES
        .iter()
        .copied()
        .filter(|name| !GRAY_FAMILIES.contains(name))
        .collect();

    let _ = writeln!(
        out,
        "\n/// The Radix scales a host or widget color can come from, in Radix's order."
    );
    let _ = writeln!(
        out,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\npub enum Scale {{"
    );
    for name in &chromatic {
        let _ = writeln!(out, "    {},", variant(name));
    }
    let _ = writeln!(
        out,
        "}}\n\nimpl Scale {{\n    pub const ALL: [Scale; {}] = [",
        chromatic.len()
    );
    for name in &chromatic {
        let _ = writeln!(out, "        Scale::{},", variant(name));
    }
    let _ = writeln!(
        out,
        "    ];\n\n    /// Radix's name for the scale.\n    pub const fn name(self) -> &'static str {{\n        match self {{"
    );
    for name in &chromatic {
        let _ = writeln!(
            out,
            "            Scale::{} => {},",
            variant(name),
            literal(name)
        );
    }
    let _ = writeln!(out, "        }}\n    }}\n}}");

    let _ = writeln!(out, "\n/// The Radix gray families.");
    let _ = writeln!(
        out,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\npub enum GrayFamily {{"
    );
    for name in GRAY_FAMILIES {
        let _ = writeln!(out, "    {},", variant(name));
    }
    let _ = writeln!(
        out,
        "}}\n\nimpl GrayFamily {{\n    pub const ALL: [GrayFamily; {}] = [",
        GRAY_FAMILIES.len()
    );
    for name in GRAY_FAMILIES {
        let _ = writeln!(out, "        GrayFamily::{},", variant(name));
    }
    let _ = writeln!(
        out,
        "    ];\n\n    pub const fn name(self) -> &'static str {{\n        match self {{"
    );
    for name in GRAY_FAMILIES {
        let _ = writeln!(
            out,
            "            GrayFamily::{} => {},",
            variant(name),
            literal(name)
        );
    }
    let _ = writeln!(out, "        }}\n    }}\n}}");

    let _ = writeln!(
        out,
        "\n/// OKLCH hue in degrees and chroma of light step 9, per `Scale::ALL`."
    );
    let hues: Vec<String> = chromatic
        .iter()
        .map(|n| format!("{:.4}", palette.scales[*n].hue()))
        .collect();
    let chromas: Vec<String> = chromatic
        .iter()
        .map(|n| format!("{:.4}", palette.scales[*n].chroma()))
        .collect();
    let _ = writeln!(
        out,
        "pub(crate) const HUE: [f64; {}] = [{}];",
        chromatic.len(),
        hues.join(", ")
    );
    let _ = writeln!(
        out,
        "pub(crate) const CHROMA: [f64; {}] = [{}];",
        chromatic.len(),
        chromas.join(", ")
    );

    let _ = writeln!(out, "\n/// picks: the hue rule and what counts as neutral.");
    let _ = writeln!(
        out,
        "pub(crate) const HUE_DISTANCE: f64 = {};",
        float(design.picks.hue_distance)
    );
    let _ = writeln!(
        out,
        "pub(crate) const NEUTRAL_BELOW_CHROMA: f64 = {};",
        float(design.picks.neutral_below_chroma)
    );
    let status: Vec<String> = design
        .status
        .iter()
        .map(|s| format!("Scale::{}", variant(s)))
        .collect();
    let _ = writeln!(
        out,
        "/// scales.status: ok, warning, error.\npub(crate) const STATUS: [Scale; 3] = [{}];",
        status.join(", ")
    );
    let default = &design.picks.default;
    let _ = writeln!(
        out,
        "/// picks.default.\npub(crate) const DEFAULT_GRAY: GrayFamily = GrayFamily::{};\npub(crate) const DEFAULT_HOST: Option<Scale> = {};\npub(crate) const DEFAULT_WIDGET: Scale = Scale::{};\npub(crate) const DEFAULT_RADIUS: crate::Radius = crate::Radius::{};\npub(crate) const DEFAULT_THEME: crate::ThemePick = crate::ThemePick::{};",
        variant(&default.gray),
        default.host.as_ref().map_or("None".to_owned(), |h| format!(
            "Some(Scale::{})",
            variant(h)
        )),
        variant(&default.widget),
        variant(&default.radius),
        variant(&default.theme)
    );

    let _ = writeln!(
        out,
        "\n/// The text color a scale's solid steps take.\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub(crate) enum SolidText {{\n    White,\n    Dark,\n    None,\n}}"
    );
    let solid: Vec<String> = chromatic
        .iter()
        .map(|n| {
            match design.solid_text.get(*n) {
                Some(SolidText::White) => "SolidText::White",
                Some(SolidText::Dark) => "SolidText::Dark",
                _ => "SolidText::None",
            }
            .to_owned()
        })
        .collect();
    let _ = writeln!(
        out,
        "/// scales.solid-text, per `Scale::ALL`.\npub(crate) const SOLID_TEXT: [SolidText; {}] = [{}];",
        chromatic.len(),
        solid.join(", ")
    );

    let _ = writeln!(
        out,
        "\n/// Radix 3.0.0, light and dark, per `Scale::ALL` and `GrayFamily::ALL`."
    );
    let rows = |names: &[&str], theme: Theme| -> Vec<(String, [crate::color::Rgb; 12])> {
        names
            .iter()
            .map(|n| {
                (
                    (*n).to_owned(),
                    match theme {
                        Theme::Light => palette.scales[*n].light,
                        Theme::Dark => palette.scales[*n].dark,
                    },
                )
            })
            .collect()
    };
    for (table, names, theme) in [
        ("SCALE_LIGHT", chromatic.as_slice(), Theme::Light),
        ("SCALE_DARK", chromatic.as_slice(), Theme::Dark),
        ("GRAY_LIGHT", GRAY_FAMILIES, Theme::Light),
        ("GRAY_DARK", GRAY_FAMILIES, Theme::Dark),
    ] {
        let data = rows(names, theme);
        let borrowed: Vec<(&str, [crate::color::Rgb; 12])> =
            data.iter().map(|(n, s)| (n.as_str(), *s)).collect();
        rgb_table(&mut out, table, &borrowed);
    }
    let alphas: Vec<String> = palette.black_a.iter().map(|c| float(c.a)).collect();
    let _ = writeln!(
        out,
        "/// blackA1-12: black at these opacities.\npub(crate) const BLACK_A: [f64; 12] = [{}];",
        alphas.join(", ")
    );

    let _ = writeln!(out, "\n{ROLE_TYPES}");
    roles(&mut out, design, false);

    let _ = writeln!(
        out,
        "\n/// The built-in status icons, SVG path data on a 16-unit grid.\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum Icon {{"
    );
    for (name, _) in &design.icons {
        let _ = writeln!(out, "    {},", variant(name));
    }
    let _ = writeln!(
        out,
        "}}\n\nimpl Icon {{\n    /// The icon's SVG path data, drawn with the even-odd rule on a 16 × 16 grid.\n    pub const fn path(self) -> &'static str {{\n        match self {{"
    );
    for (name, path) in &design.icons {
        let _ = writeln!(
            out,
            "            Icon::{} => {},",
            variant(name),
            literal(path)
        );
    }
    let _ = writeln!(out, "        }}\n    }}\n}}");

    let _ = writeln!(
        out,
        "\n/// A tone as roles.\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub(crate) struct ToneDef {{\n    pub(crate) text: &'static str,\n    pub(crate) surface: &'static str,\n    pub(crate) border: Option<&'static str>,\n    pub(crate) icon: Option<&'static str>,\n    pub(crate) status_icon: Option<Icon>,\n}}\n\n/// tones: default, accent, ok, warning, error.\npub(crate) const TONES: [ToneDef; 5] = ["
    );
    let option = |value: &Option<String>| {
        value
            .as_ref()
            .map_or("None".to_owned(), |v| format!("Some({})", literal(v)))
    };
    for tone in &design.tones {
        let icon = tone
            .status_icon
            .as_ref()
            .map_or("None".to_owned(), |i| format!("Some(Icon::{})", variant(i)));
        let _ = writeln!(
            out,
            "    // {}\n    ToneDef {{ text: {}, surface: {}, border: {}, icon: {}, status_icon: {icon} }},",
            tone.name.name(),
            literal(&tone.text),
            literal(&tone.surface),
            option(&tone.border),
            option(&tone.icon)
        );
    }
    let _ = writeln!(out, "];");

    let _ = writeln!(
        out,
        "\n/// The type scale; sizes in pixels.\npub mod typography {{\n    /// A type level.\n    #[derive(Clone, Copy, Debug, PartialEq)]\n    pub struct Level {{\n        pub family: &'static str,\n        pub size: f32,\n        pub weight: u16,\n        pub line_height: f32,\n        pub letter_spacing_em: f32,\n    }}\n"
    );
    for (name, level) in &design.typography {
        doc(
            &mut out,
            "    ",
            &format!("typography.{name}"),
            design
                .rationale
                .get(&format!("typography.{name}"))
                .map_or("", String::as_str),
        );
        let _ = writeln!(
            out,
            "    pub const {}: Level = Level {{ family: {}, size: {}, weight: {}, line_height: {}, letter_spacing_em: {} }};",
            constant(name),
            literal(&level.family),
            float(level.size),
            level.weight,
            float(level.line_height),
            float(level.letter_spacing_em.unwrap_or(0.0))
        );
    }
    let _ = writeln!(out, "}}");

    for (module, title, values, group) in [
        (
            "spacing",
            "The spacing scale, in pixels.",
            &design.spacing,
            "spacing",
        ),
        (
            "radius",
            "The corner radii, in pixels; the person picks one of them.",
            &design.rounded,
            "rounded",
        ),
    ] {
        let _ = writeln!(out, "\n/// {title}\npub mod {module} {{");
        for (name, value) in values {
            doc(
                &mut out,
                "    ",
                &format!("{group}.{name}"),
                design
                    .rationale
                    .get(&format!("{group}.{name}"))
                    .map_or("", String::as_str),
            );
            let _ = writeln!(
                out,
                "    pub const {}: f32 = {};",
                constant(name),
                float(*value)
            );
        }
        let _ = writeln!(out, "}}");
    }
    let picks: Vec<String> = design.radius_picks.iter().map(|n| literal(n)).collect();
    let _ = writeln!(
        out,
        "\n/// radius-picks.\npub(crate) const RADIUS_PICKS: [&str; {}] = [{}];",
        picks.len(),
        picks.join(", ")
    );

    let _ = writeln!(
        out,
        "\n/// Elevation levels the kit may use.\npub mod elevation {{\n{SHADOW_TYPE}"
    );
    elevation(&mut out, design, false);
    let _ = writeln!(out, "}}");
    out
}

fn host_surfaces_rs(design: &Design) -> String {
    let mut out = String::from(HEADER);
    let _ = writeln!(out, "\n{ROLE_TYPES}");
    roles(&mut out, design, true);
    let _ = writeln!(
        out,
        "\n/// Elevation levels only the host draws.\npub mod elevation {{\n{SHADOW_TYPE}"
    );
    elevation(&mut out, design, true);
    let _ = writeln!(out, "}}");
    let names: Vec<String> = design
        .components
        .iter()
        .filter(|c| c.host_surface)
        .map(|c| literal(&c.name))
        .collect();
    let _ = writeln!(
        out,
        "\n/// host-surfaces: the components only the host draws.\npub(crate) const COMPONENTS: [&str; {}] = [{}];",
        names.len(),
        names.join(", ")
    );
    out
}

/// A JSON string literal.
fn json(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

const HOST_MARK: &str = ", \"$extensions\": { \"dev.dashboard\": { \"hostSurface\": true } }";

fn dtcg(design: &Design, palette: &Palette) -> String {
    let appearance = design.default_appearance();
    let mut groups: Vec<String> = Vec::new();

    let mut themes = Vec::new();
    for theme in Theme::BOTH {
        let mut tokens = Vec::new();
        for spec in &design.roles {
            let color = design
                .resolve(palette, &spec.role, theme, appearance)
                .expect("checked")
                .to_hex();
            let mark = if spec.role.host_surface {
                HOST_MARK
            } else {
                ""
            };
            tokens.push(format!(
                "      {}: {{ \"$type\": \"color\", \"$value\": {}, \"$description\": {}{mark} }}",
                json(&spec.role.name),
                json(&color),
                json(&spec.role.why)
            ));
        }
        themes.push(format!(
            "    {}: {{\n{}\n    }}",
            json(theme.name()),
            tokens.join(",\n")
        ));
    }
    groups.push(format!("  \"color\": {{\n{}\n  }}", themes.join(",\n")));

    let why = |path: &str| design.rationale.get(path).cloned().unwrap_or_default();
    let mut tokens = Vec::new();
    for (name, level) in &design.typography {
        let spacing = level.letter_spacing_em.map_or(String::new(), |em| {
            format!(", \"letterSpacing\": {}", json(&format!("{em}em")))
        });
        tokens.push(format!(
            "    {}: {{ \"$type\": \"typography\", \"$value\": {{ \"fontFamily\": {}, \"fontSize\": {}, \"fontWeight\": {}, \"lineHeight\": {}{spacing} }}, \"$description\": {} }}",
            json(name),
            json(&level.family),
            json(&format!("{}px", level.size)),
            level.weight,
            json(&format!("{}px", level.line_height)),
            json(&why(&format!("typography.{name}")))
        ));
    }
    groups.push(format!(
        "  \"typography\": {{\n{}\n  }}",
        tokens.join(",\n")
    ));

    for (group, values, source) in [
        ("spacing", &design.spacing, "spacing"),
        ("radius", &design.rounded, "rounded"),
    ] {
        let tokens: Vec<String> = values
            .iter()
            .map(|(name, value)| {
                format!(
                    "    {}: {{ \"$type\": \"dimension\", \"$value\": {}, \"$description\": {} }}",
                    json(name),
                    json(&format!("{value}px")),
                    json(&why(&format!("{source}.{name}")))
                )
            })
            .collect();
        groups.push(format!(
            "  {}: {{\n{}\n  }}",
            json(group),
            tokens.join(",\n")
        ));
    }

    let mut tokens = Vec::new();
    for level in &design.elevation {
        let Some(shadow) = level.shadow else { continue };
        let alpha = palette.black_a[usize::from(shadow.black_a) - 1].a;
        let color = format!("#000000{:02x}", (alpha * 255.0).round() as u8);
        let mark = if level.host_surface { HOST_MARK } else { "" };
        tokens.push(format!(
            "    {}: {{ \"$type\": \"shadow\", \"$value\": {{ \"color\": {}, \"offsetX\": {}, \"offsetY\": {}, \"blur\": {}, \"spread\": \"0px\" }}, \"$description\": {}{mark} }}",
            json(&level.name),
            json(&color),
            json(&format!("{}px", shadow.x)),
            json(&format!("{}px", shadow.y)),
            json(&format!("{}px", shadow.blur)),
            json(&level.why)
        ));
    }
    groups.push(format!("  \"shadow\": {{\n{}\n  }}", tokens.join(",\n")));

    format!(
        "{{\n  \"$description\": {},\n{}\n}}\n",
        json(
            "Generated from DESIGN.md by `cargo xtask design`. Do not edit. Colors are the default appearance."
        ),
        groups.join(",\n")
    )
}

/// Every generated file, from a checked design.
pub fn generate(design: &Design, palette: &Palette) -> Generated {
    Generated {
        files: vec![
            (TOKENS_RS.to_owned(), tokens_rs(design, palette)),
            (HOST_SURFACES_RS.to_owned(), host_surfaces_rs(design)),
            (DTCG_JSON.to_owned(), dtcg(design, palette)),
        ],
    }
}

/// How the committed files differ from `generated`, one line per file, naming the first token
/// that differs (the nearest `/// <path>:` comment above the line, or the JSON key).
pub fn differences(root: &Path, generated: &Generated) -> Vec<String> {
    let mut out = Vec::new();
    for (path, expected) in &generated.files {
        let committed = match std::fs::read_to_string(root.join(path)) {
            Ok(text) => text.replace("\r\n", "\n"),
            Err(error) => {
                out.push(format!("{path}: {error}"));
                continue;
            }
        };
        if &committed == expected {
            continue;
        }
        let expected_lines: Vec<&str> = expected.lines().collect();
        let committed_lines: Vec<&str> = committed.lines().collect();
        let index = expected_lines
            .iter()
            .zip(&committed_lines)
            .position(|(a, b)| a != b)
            .unwrap_or(expected_lines.len().min(committed_lines.len()));
        let token = expected_lines[..=index.min(expected_lines.len().saturating_sub(1))]
            .iter()
            .rev()
            .find_map(|line| {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed
                    .strip_prefix("/// ")
                    .or_else(|| trimmed.strip_prefix("// "))
                {
                    rest.split_once(':')
                        .map(|(path, _)| path.to_owned())
                        .filter(|p| p.contains('.'))
                } else {
                    trimmed
                        .strip_prefix('"')
                        .and_then(|rest| rest.split_once('"'))
                        .map(|(key, _)| key.to_owned())
                }
            })
            .unwrap_or_else(|| "the file's layout".to_owned());
        out.push(format!(
            "{path}:{}: {token} differs from what DESIGN.md generates",
            index + 1
        ));
    }
    out
}

/// Checks `DESIGN.md` and compares the committed files with what it generates.
pub fn compare(root: &Path) -> Result<(), Vec<String>> {
    let (design, palette) = crate::load(root)
        .map_err(|problems| problems.iter().map(ToString::to_string).collect::<Vec<_>>())?;
    let differences = differences(root, &generate(&design, &palette));
    if differences.is_empty() {
        Ok(())
    } else {
        Err(differences)
    }
}

/// Writes every generated file.
pub fn write(root: &Path, generated: &Generated) -> std::io::Result<()> {
    for (path, text) in &generated.files {
        std::fs::write(root.join(path), text)?;
    }
    Ok(())
}
