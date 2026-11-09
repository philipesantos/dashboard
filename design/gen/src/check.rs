//! The checker (specs/007-host-visual-design/contracts/design-md.md): builds the `Design` from a
//! parsed `DESIGN.md` and the Radix palette, refusing every problem it finds with the line of
//! `DESIGN.md` and the token path. Rules 10, 11 and 13 (contrast) are in `lint`, run once the
//! rest pass.

use crate::color::{Rgb, Rgba};
use crate::model::{PairingKind, Role, RoleSource, Step, Tone, ToneName};
use crate::radix::{GRAY_FAMILIES, Kind, Palette, STEPS, Theme};
use crate::source::{Block, SECTION_ORDER, Source, canonical_position};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use yaml_rust2::Yaml;

/// One refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub line: usize,
    /// The token path, such as `typography.body-md.fontSize`; empty when the problem is the file's.
    pub path: String,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = escape(&self.message);
        if self.path.is_empty() {
            write!(formatter, "DESIGN.md:{}: {message}", self.line)
        } else {
            write!(
                formatter,
                "DESIGN.md:{}: {}: {message}",
                self.line,
                escape(&self.path)
            )
        }
    }
}

/// Control characters escaped, so nothing in DESIGN.md can rewrite the terminal's output.
fn escape(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

/// A role and how it is checked for contrast (data-model.md, Role and Pairing).
#[derive(Clone, Debug, PartialEq)]
pub struct RoleSpec {
    pub role: Role,
    /// What it is checked as when drawn on the roles in `on`; `None` for a background.
    pub kind: Option<PairingKind>,
    pub on: Vec<String>,
    /// The WCAG exemption a `decorative` or `inactive` role rests on.
    pub exempt: Option<String>,
}

/// The text color a scale's solid steps take (FR-009).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolidText {
    White,
    Dark,
    None,
}

/// The default appearance as `DESIGN.md` writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultPick {
    pub gray: String,
    /// `None` is the monochrome host.
    pub host: Option<String>,
    pub widget: String,
    pub radius: String,
    pub theme: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Picks {
    pub gray: Vec<String>,
    pub hue_distance: f64,
    pub neutral_below_chroma: f64,
    pub default: DefaultPick,
}

/// `<x>px <y>px <blur>px blackA-<n>`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub x: f64,
    pub y: f64,
    pub blur: f64,
    pub black_a: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Elevation {
    pub name: String,
    pub shadow: Option<Shadow>,
    pub host_surface: bool,
    pub why: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    pub family: String,
    pub size: f64,
    pub weight: u16,
    /// In pixels.
    pub line_height: f64,
    pub letter_spacing_em: Option<f64>,
    pub feature: Option<String>,
}

/// A component property's value.
#[derive(Clone, Debug, PartialEq)]
pub enum PropValue {
    Reference { group: String, token: String },
    Dimension(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub name: String,
    pub props: Vec<(String, PropValue)>,
    pub host_surface: bool,
}

/// A role's color in one theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Opaque(Rgb),
    Translucent(Rgba),
}

impl Color {
    /// `#rrggbb`, or `#rrggbbaa` for a translucent color.
    pub fn to_hex(self) -> String {
        match self {
            Color::Opaque(rgb) => rgb.to_hex(),
            Color::Translucent(rgba) => format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                rgba.r,
                rgba.g,
                rgba.b,
                (rgba.a * 255.0).round() as u8
            ),
        }
    }
}

/// The picks a resolution reads: a gray family, the host color (`None` for monochrome) and the
/// widget color, all scale names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Appearance<'a> {
    pub gray: &'a str,
    pub host: Option<&'a str>,
    pub widget: &'a str,
}

/// Everything `DESIGN.md` decides, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Design {
    pub name: String,
    pub description: String,
    /// The status scales: ok, warning, error (FR-024).
    pub status: [String; 3],
    pub solid_text: BTreeMap<String, SolidText>,
    pub picks: Picks,
    pub roles: Vec<RoleSpec>,
    pub tones: Vec<Tone>,
    pub elevation: Vec<Elevation>,
    pub radius_picks: Vec<String>,
    pub icons: Vec<(String, String)>,
    pub typography: Vec<(String, Typography)>,
    pub spacing: Vec<(String, f64)>,
    pub rounded: Vec<(String, f64)>,
    pub components: Vec<Component>,
    pub rationale: BTreeMap<String, String>,
}

impl Design {
    pub fn role(&self, name: &str) -> Option<&RoleSpec> {
        self.roles.iter().find(|spec| spec.role.name == name)
    }

    pub fn default_appearance(&self) -> Appearance<'_> {
        let default = &self.picks.default;
        Appearance {
            gray: &default.gray,
            host: default.host.as_deref(),
            widget: &default.widget,
        }
    }

    /// The scale a role's source reads under an appearance (`None` for the overlay).
    pub fn scale_of<'a>(
        &'a self,
        source: RoleSource,
        appearance: Appearance<'a>,
    ) -> Option<&'a str> {
        Some(match source {
            RoleSource::Gray => appearance.gray,
            RoleSource::Widget => appearance.widget,
            RoleSource::Host => appearance.host.unwrap_or(appearance.gray),
            RoleSource::Ok => &self.status[0],
            RoleSource::Warning => &self.status[1],
            RoleSource::Error => &self.status[2],
            RoleSource::Overlay => return None,
        })
    }

    /// A role's color in a theme under an appearance.
    pub fn resolve(
        &self,
        palette: &Palette,
        role: &Role,
        theme: Theme,
        appearance: Appearance<'_>,
    ) -> Result<Color, String> {
        let step = match theme {
            Theme::Light => role.light,
            Theme::Dark => role.dark,
        };
        let Some(scale_name) = self.scale_of(role.source, appearance) else {
            return match step {
                Step::Radix(n) => Ok(Color::Translucent(palette.black_a[usize::from(n) - 1])),
                _ => Err("an overlay role takes a blackA step".into()),
            };
        };
        let scale = palette
            .scales
            .get(scale_name)
            .ok_or_else(|| format!("no scale {scale_name}"))?;
        Ok(Color::Opaque(match step {
            Step::Radix(n) => scale.step(theme, n),
            Step::White => Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            Step::Black => Rgb { r: 0, g: 0, b: 0 },
            Step::SolidText => match self.solid_text.get(scale_name) {
                Some(SolidText::White) => Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                Some(SolidText::Dark) => palette.scales[appearance.gray].step(Theme::Light, 12),
                _ => {
                    return Err(format!(
                        "{scale_name} declares no text color for its solid steps"
                    ));
                }
            },
        }))
    }

    /// How many tokens the DTCG export marks as host surfaces: each host-surface role in both
    /// themes, and each host-surface elevation level that has a shadow (a flat level exports no
    /// token).
    pub fn host_surface_count(&self) -> usize {
        2 * self
            .roles
            .iter()
            .filter(|spec| spec.role.host_surface)
            .count()
            + self
                .elevation
                .iter()
                .filter(|level| level.host_surface && level.shadow.is_some())
                .count()
    }
}

/// The checker's state while it reads.
struct Checker<'a> {
    source: &'a Source,
    palette: &'a Palette,
    problems: Vec<Problem>,
}

const FORMAT_KEYS: &[&str] = &[
    "version",
    "name",
    "description",
    "omitted",
    "colors",
    "typography",
    "rounded",
    "spacing",
    "components",
];
const PROJECT_KEYS: &[&str] = &[
    "scales",
    "picks",
    "roles",
    "tones",
    "elevation",
    "radius-picks",
    "icons",
    "host-surfaces",
    "rationale",
];
const TYPOGRAPHY_KEYS: &[&str] = &[
    "fontFamily",
    "fontSize",
    "fontWeight",
    "lineHeight",
    "letterSpacing",
    "fontFeature",
    "fontVariation",
];
const COMPONENT_PROPS: &[&str] = &[
    "backgroundColor",
    "textColor",
    "typography",
    "rounded",
    "padding",
    "size",
    "height",
    "width",
];

fn text(yaml: &Yaml) -> Option<&str> {
    yaml.as_str()
}

fn number(yaml: &Yaml) -> Option<f64> {
    match yaml {
        Yaml::Integer(n) => Some(*n as f64),
        Yaml::Real(_) => yaml.as_f64(),
        _ => None,
    }
}

/// `12px`, `-0.02em`: the number and its unit.
fn dimension(yaml: &Yaml) -> Option<(f64, &str)> {
    let value = yaml.as_str()?;
    let unit_at = value.find(|c: char| c.is_ascii_alphabetic())?;
    let (digits, unit) = value.split_at(unit_at);
    if !["px", "em", "rem"].contains(&unit) || digits.is_empty() {
        return None;
    }
    let parsed: f64 = digits.parse().ok()?;
    parsed.is_finite().then_some((parsed, unit))
}

fn px(yaml: &Yaml) -> Option<f64> {
    dimension(yaml)
        .filter(|(_, unit)| *unit == "px")
        .map(|(value, _)| value)
}

fn entries(yaml: &Yaml) -> Vec<(String, &Yaml)> {
    yaml.as_hash()
        .map(|hash| {
            hash.iter()
                .map(|(key, value)| {
                    (
                        key.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("{key:?}")),
                        value,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn is_kebab(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

impl<'a> Checker<'a> {
    fn problem(&mut self, line: usize, path: impl Into<String>, message: impl Into<String>) {
        self.problems.push(Problem {
            line,
            path: path.into(),
            message: message.into(),
        });
    }

    /// The front-matter line of a nested path, such as `["typography", "body-md", "fontSize"]`.
    fn front_line(&self, path: &[&str]) -> usize {
        let mut from = 0;
        let mut found = 1;
        for segment in path {
            let wanted = format!("{segment}:");
            match self.source.front_lines[from..]
                .iter()
                .position(|line| line.trim_start().starts_with(&wanted))
            {
                Some(offset) => {
                    from += offset;
                    found = from + 2;
                    from += 1;
                }
                None => break,
            }
        }
        found
    }

    fn block_line(block: &Block, path: &[&str]) -> usize {
        let mut from = 0;
        let mut found = block.line;
        for segment in path {
            let wanted = format!("{segment}:");
            let hit = block.lines[from..].iter().position(|line| {
                let trimmed = line
                    .trim_start()
                    .trim_start_matches("- ")
                    .trim_start_matches('{')
                    .trim_start();
                trimmed.starts_with(&wanted)
                    || line.contains(&format!(" {wanted}"))
                    || line.contains(&format!("{{{wanted}"))
            });
            match hit {
                Some(offset) => {
                    from += offset;
                    found = block.line + 1 + from;
                }
                None => break,
            }
        }
        found
    }

    fn roles_key(&mut self, key: &str) -> Option<(&'a Yaml, &'a Block)> {
        let found = self.source.roles(key);
        if found.is_none() {
            self.problem(
                1,
                key,
                format!("the project layer has no `{key}` (a design-roles block)"),
            );
        }
        found
    }

    // Rule 1: the front matter's keys and types; rule 3: the sections' order.
    fn format_layer(&mut self) -> (String, String) {
        let front = &self.source.front;
        for (key, _) in entries(front) {
            if !FORMAT_KEYS.contains(&key.as_str()) {
                let line = self.source.front_line_of(&key);
                self.problem(line, &key, "is not a key of the DESIGN.md format 0.4.0");
            }
        }
        if text(&front["version"]) != Some("alpha") {
            let line = self.source.front_line_of("version");
            self.problem(
                line,
                "version",
                "must be the string `alpha`, the format version 0.4.0 names",
            );
        }
        let name = text(&front["name"]).unwrap_or_default().to_owned();
        if name.is_empty() {
            self.problem(1, "name", "the front matter needs a name");
        }
        let description = match &front["description"] {
            Yaml::BadValue => String::new(),
            value => text(value).map(str::to_owned).unwrap_or_else(|| {
                let line = self.source.front_line_of("description");
                self.problem(line, "description", "must be a string");
                String::new()
            }),
        };
        let mut last: Option<(usize, String)> = None;
        let mut present = BTreeSet::new();
        for section in &self.source.sections {
            let Some(position) = canonical_position(&section.title) else {
                continue;
            };
            present.insert(position);
            if let Some((previous, title)) = &last
                && position < *previous
            {
                let message = format!("comes after {title:?}, against the format's section order");
                self.problems.push(Problem {
                    line: section.line,
                    path: section.title.clone(),
                    message,
                });
            }
            last = Some((position, section.title.clone()));
        }
        for (position, names) in SECTION_ORDER.iter().enumerate() {
            if !present.contains(&position) {
                self.problem(1, names[0], "this section is missing");
            }
        }
        (name, description)
    }

    fn typography(&mut self) -> Vec<(String, Typography)> {
        let mut levels = Vec::new();
        for (name, value) in entries(&self.source.front["typography"]) {
            let bad = |checker: &mut Self, field: &str, message: &str| {
                let line = checker.front_line(&["typography", &name, field]);
                checker.problem(
                    line,
                    format!("typography.{name}.{field}"),
                    message.to_owned(),
                );
            };
            for (field, _) in entries(value) {
                if !TYPOGRAPHY_KEYS.contains(&field.as_str()) {
                    bad(self, &field, "is not a typography property of the format");
                }
            }
            let family = text(&value["fontFamily"]).unwrap_or_default().to_owned();
            if family.is_empty() {
                bad(self, "fontFamily", "is required, a string");
            }
            let size = px(&value["fontSize"]);
            if size.is_none() {
                bad(
                    self,
                    "fontSize",
                    "must be a dimension in px, such as `14px`",
                );
            }
            let weight = number(&value["fontWeight"])
                .or_else(|| text(&value["fontWeight"]).and_then(|w| w.parse().ok()))
                .filter(|w| (100.0..=900.0).contains(w) && w.fract() == 0.0);
            if weight.is_none() {
                bad(self, "fontWeight", "must be a weight from 100 to 900");
            }
            let line_height = px(&value["lineHeight"])
                .or_else(|| number(&value["lineHeight"]).and_then(|n| size.map(|s| n * s)));
            if line_height.is_none() {
                bad(
                    self,
                    "lineHeight",
                    "must be a dimension in px or a multiple of the font size",
                );
            }
            let letter_spacing_em = match &value["letterSpacing"] {
                Yaml::BadValue => None,
                spacing => match dimension(spacing) {
                    Some((value, "em")) => Some(value),
                    _ => {
                        bad(
                            self,
                            "letterSpacing",
                            "must be a dimension in em, such as `-0.01em`",
                        );
                        None
                    }
                },
            };
            let feature = text(&value["fontFeature"]).map(str::to_owned);
            if let (Some(size), Some(weight), Some(line_height)) = (size, weight, line_height) {
                levels.push((
                    name.clone(),
                    Typography {
                        family,
                        size,
                        weight: weight as u16,
                        line_height,
                        letter_spacing_em,
                        feature,
                    },
                ));
            }
        }
        levels
    }

    fn dimensions(&mut self, group: &str, allow_number: bool) -> Vec<(String, f64)> {
        let mut values = Vec::new();
        for (name, value) in entries(&self.source.front[group]) {
            match px(value).or_else(|| allow_number.then(|| number(value)).flatten()) {
                Some(px) if px >= 0.0 => values.push((name, px)),
                _ => {
                    let line = self.front_line(&[group, &name]);
                    self.problem(
                        line,
                        format!("{group}.{name}"),
                        "must be a non-negative dimension in px",
                    );
                }
            }
        }
        values
    }

    // Rules 2, 12 and 14 on components.
    fn components(
        &mut self,
        typography: &[(String, Typography)],
        roles: &BTreeSet<String>,
        host_roles: &BTreeSet<String>,
    ) -> Vec<Component> {
        let host_list = self.host_surface_components();
        let mut components = Vec::new();
        for (name, value) in entries(&self.source.front["components"]) {
            let host_surface = host_list.contains(&name);
            let mut props = Vec::new();
            for (prop, raw) in entries(value) {
                let line = self.front_line(&["components", &name, &prop]);
                let path = format!("components.{name}.{prop}");
                let lower = prop.to_ascii_lowercase();
                if ["image", "url", "src", "icon"]
                    .iter()
                    .any(|word| lower.contains(word))
                {
                    self.problem(
                        line,
                        &path,
                        "no component takes an image, an icon source or a URL (FR-023)",
                    );
                    continue;
                }
                if !COMPONENT_PROPS.contains(&prop.as_str()) {
                    self.problem(line, &path, "is not a component property of the format");
                    continue;
                }
                let Some(literal) = text(raw) else {
                    self.problem(
                        line,
                        &path,
                        "must be a string: a {reference} or a dimension",
                    );
                    continue;
                };
                if let Some(reference) = literal
                    .strip_prefix('{')
                    .and_then(|rest| rest.strip_suffix('}'))
                {
                    let Some((group, token)) = reference.split_once('.') else {
                        self.problem(
                            line,
                            &path,
                            format!("{{{reference}}} is not a {{group.token}} reference"),
                        );
                        continue;
                    };
                    let expected = match prop.as_str() {
                        "backgroundColor" | "textColor" => "colors",
                        "typography" => "typography",
                        "rounded" => "rounded",
                        _ => "spacing",
                    };
                    let exists = match group {
                        // A color reference names a role; the front matter's colors are those roles resolved
                        "colors" => roles.contains(token.strip_suffix("-dark").unwrap_or(token)),
                        "typography" => typography.iter().any(|(level, _)| level == token),
                        "rounded" | "spacing" => !self.source.front[group][token].is_badvalue(),
                        _ => false,
                    };
                    if !exists || group != expected {
                        self.problem(
                            line,
                            &path,
                            format!("{{{reference}}} does not resolve to a {expected} token"),
                        );
                        continue;
                    }
                    let role = token.strip_suffix("-dark").unwrap_or(token);
                    if group == "colors" && host_roles.contains(role) && !host_surface {
                        self.problem(
                            line,
                            &path,
                            format!("references the host surface {role}, but {name} is not a host surface"),
                        );
                    }
                    props.push((
                        prop,
                        PropValue::Reference {
                            group: group.into(),
                            token: token.into(),
                        },
                    ));
                } else if let Some(value) = px(raw) {
                    props.push((prop, PropValue::Dimension(value)));
                } else {
                    self.problem(line, &path, "must be a {reference} or a dimension in px");
                }
            }
            components.push(Component {
                name,
                props,
                host_surface,
            });
        }
        components
    }

    fn host_surface_components(&mut self) -> Vec<String> {
        let Some((value, block)) = self.source.roles("host-surfaces") else {
            return Vec::new();
        };
        let names: Vec<String> = value
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(|n| n.as_str().map(str::to_owned))
            .collect();
        for name in &names {
            if self.source.front["components"][name.as_str()].is_badvalue() {
                let line = Self::block_line(block, &["host-surfaces"]);
                self.problem(
                    line,
                    format!("host-surfaces.{name}"),
                    "is not a component of the front matter",
                );
            }
        }
        names
    }

    // Rule 4.
    fn project_keys(&mut self) {
        for block in &self.source.blocks {
            for (key, _) in entries(&block.yaml) {
                if !PROJECT_KEYS.contains(&key.as_str()) {
                    let line = block.line_of(&key);
                    self.problems.push(Problem {
                        line,
                        path: key,
                        message: "is not a key of the project layer".into(),
                    });
                }
            }
        }
    }

    fn scale_exists(&self, name: &str) -> bool {
        self.palette.scales.contains_key(name)
    }

    fn scales(&mut self) -> ([String; 3], BTreeMap<String, SolidText>) {
        let mut status = [String::new(), String::new(), String::new()];
        let mut solid = BTreeMap::new();
        let Some((value, block)) = self.roles_key("scales") else {
            return (status, solid);
        };
        if text(&value["radix"]) != Some("3.0.0") {
            let line = Self::block_line(block, &["radix"]);
            self.problem(
                line,
                "scales.radix",
                "must be \"3.0.0\", the version vendored under design/radix/",
            );
        }
        for (index, tone) in ["ok", "warning", "error"].iter().enumerate() {
            let name = text(&value["status"][*tone]).unwrap_or_default();
            if !self.scale_exists(name) || GRAY_FAMILIES.contains(&name) {
                let line = Self::block_line(block, &["status", tone]);
                self.problem(
                    line,
                    format!("scales.status.{tone}"),
                    "must name a chromatic Radix scale",
                );
            }
            status[index] = name.to_owned();
        }
        for (scale, choice) in entries(&value["solid-text"]) {
            let line = Self::block_line(block, &["solid-text", &scale]);
            let parsed = match text(choice) {
                Some("white") => SolidText::White,
                Some("dark") => SolidText::Dark,
                Some("none") => SolidText::None,
                _ => {
                    self.problem(
                        line,
                        format!("scales.solid-text.{scale}"),
                        "must be white, dark or none",
                    );
                    continue;
                }
            };
            if !self.scale_exists(&scale) || GRAY_FAMILIES.contains(&scale.as_str()) {
                self.problem(
                    line,
                    format!("scales.solid-text.{scale}"),
                    "is not a chromatic Radix scale",
                );
            }
            solid.insert(scale, parsed);
        }
        (status, solid)
    }

    fn picks(&mut self, radius_names: &[String]) -> Picks {
        let fallback = Picks {
            gray: Vec::new(),
            hue_distance: 30.0,
            neutral_below_chroma: 0.08,
            default: DefaultPick {
                gray: "gray".into(),
                host: None,
                widget: "blue".into(),
                radius: "medium".into(),
                theme: "system".into(),
            },
        };
        let Some((value, block)) = self.roles_key("picks") else {
            return fallback;
        };
        let gray: Vec<String> = value["gray"]
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(|g| g.as_str().map(str::to_owned))
            .collect();
        for name in &gray {
            if !GRAY_FAMILIES.contains(&name.as_str()) {
                let line = Self::block_line(block, &["gray"]);
                self.problem(
                    line,
                    format!("picks.gray.{name}"),
                    "is not a Radix gray family",
                );
            }
        }
        let hue_distance = number(&value["hue-distance"]).unwrap_or_else(|| {
            let line = Self::block_line(block, &["hue-distance"]);
            self.problem(line, "picks.hue-distance", "must be a number of degrees");
            30.0
        });
        let neutral_below_chroma = number(&value["neutral-below-chroma"]).unwrap_or_else(|| {
            let line = Self::block_line(block, &["neutral-below-chroma"]);
            self.problem(line, "picks.neutral-below-chroma", "must be a number");
            0.08
        });
        let default = &value["default"];
        let field = |name: &str| text(&default[name]).unwrap_or_default().to_owned();
        let pick = DefaultPick {
            gray: field("gray"),
            host: match field("host").as_str() {
                "monochrome" => None,
                other => Some(other.to_owned()),
            },
            widget: field("widget"),
            radius: field("radius"),
            theme: field("theme"),
        };
        let line = Self::block_line(block, &["default"]);
        if !gray.contains(&pick.gray) {
            self.problem(line, "picks.default.gray", "must be one of picks.gray");
        }
        if let Some(host) = &pick.host
            && !self.scale_exists(host)
        {
            self.problem(
                line,
                "picks.default.host",
                "must be monochrome or a Radix scale",
            );
        }
        if !self.scale_exists(&pick.widget) {
            self.problem(line, "picks.default.widget", "must be a Radix scale");
        }
        if !radius_names.contains(&pick.radius) {
            self.problem(line, "picks.default.radius", "must be one of radius-picks");
        }
        if !["light", "dark", "system"].contains(&pick.theme.as_str()) {
            self.problem(line, "picks.default.theme", "must be light, dark or system");
        }
        Picks {
            gray,
            hue_distance,
            neutral_below_chroma,
            default: pick,
        }
    }

    fn step(&mut self, yaml: &Yaml, line: usize, path: &str, overlay: bool) -> Option<Step> {
        let step = match yaml {
            Yaml::Integer(n) if (1..=STEPS as i64).contains(n) => Some(Step::Radix(*n as u8)),
            Yaml::String(word) if !overlay => match word.as_str() {
                "white" => Some(Step::White),
                "black" => Some(Step::Black),
                "solid-text" => Some(Step::SolidText),
                _ => None,
            },
            _ => None,
        };
        if step.is_none() {
            let allowed = if overlay {
                "a blackA step 1-12"
            } else {
                "a step 1-12, white, black or solid-text"
            };
            self.problem(line, path, format!("must be {allowed}"));
        }
        step
    }

    // Rules 5 and 7.
    fn roles(&mut self) -> Vec<RoleSpec> {
        let Some((value, block)) = self.roles_key("roles") else {
            return Vec::new();
        };
        let mut roles = Vec::new();
        for (name, fields) in entries(value) {
            let line = Self::block_line(block, &[&name]);
            if !is_kebab(&name) {
                self.problem(line, format!("roles.{name}"), "a role's name is kebab-case");
            }
            for (field, _) in entries(fields) {
                if ![
                    "source",
                    "light",
                    "dark",
                    "why",
                    "host-surface",
                    "kind",
                    "on",
                    "exempt",
                ]
                .contains(&field.as_str())
                {
                    self.problem(
                        line,
                        format!("roles.{name}.{field}"),
                        "is not a field of a role",
                    );
                }
            }
            let source = text(&fields["source"]).and_then(RoleSource::parse);
            if source.is_none() {
                let given = text(&fields["source"]).unwrap_or("nothing");
                self.problem(
                    line,
                    format!("roles.{name}.source"),
                    format!(
                        "{given} is not a source: gray, widget, host, ok, warning, error or overlay"
                    ),
                );
            }
            let overlay = source == Some(RoleSource::Overlay);
            let light = self.step(
                &fields["light"],
                line,
                &format!("roles.{name}.light"),
                overlay,
            );
            let dark = self.step(
                &fields["dark"],
                line,
                &format!("roles.{name}.dark"),
                overlay,
            );
            let why = text(&fields["why"]).unwrap_or_default().trim().to_owned();
            if why.is_empty() {
                self.problem(
                    line,
                    format!("roles.{name}.why"),
                    "every role says why it uses its steps (FR-001)",
                );
            }
            let host_surface = fields["host-surface"].as_bool().unwrap_or(false);
            let kind = match &fields["kind"] {
                Yaml::BadValue => None,
                kind => {
                    let parsed = text(kind).and_then(PairingKind::parse);
                    if parsed.is_none() {
                        self.problem(
                            line,
                            format!("roles.{name}.kind"),
                            "must be text, large-text, non-text, decorative or inactive",
                        );
                    }
                    parsed
                }
            };
            let on: Vec<String> = fields["on"]
                .as_vec()
                .into_iter()
                .flatten()
                .filter_map(|n| n.as_str().map(str::to_owned))
                .collect();
            if kind.is_some() && on.is_empty() {
                self.problem(
                    line,
                    format!("roles.{name}.on"),
                    "a foreground role lists the roles it is drawn on",
                );
            }
            if kind.is_none() && !on.is_empty() {
                self.problem(
                    line,
                    format!("roles.{name}.kind"),
                    "a role drawn on others says what it is checked as",
                );
            }
            let exempt = text(&fields["exempt"])
                .map(str::trim)
                .filter(|e| !e.is_empty())
                .map(str::to_owned);
            if matches!(kind, Some(PairingKind::Decorative | PairingKind::Inactive))
                && exempt.is_none()
            {
                self.problem(
                    line,
                    format!("roles.{name}.exempt"),
                    "a decorative or inactive role names the WCAG exemption it rests on",
                );
            }
            if let (Some(source), Some(light), Some(dark)) = (source, light, dark) {
                roles.push(RoleSpec {
                    role: Role {
                        name,
                        source,
                        light,
                        dark,
                        why,
                        host_surface,
                    },
                    kind,
                    on,
                    exempt,
                });
            }
        }
        let names: BTreeSet<String> = roles.iter().map(|spec| spec.role.name.clone()).collect();
        for spec in &roles {
            for target in &spec.on {
                if !names.contains(target) {
                    let line = Self::block_line(block, &[&spec.role.name]);
                    self.problem(
                        line,
                        format!("roles.{}.on.{target}", spec.role.name),
                        format!("{target} is not a role"),
                    );
                }
            }
        }
        roles
    }

    // Rule 8.
    fn tones(&mut self, roles: &[RoleSpec], icons: &[(String, String)]) -> Vec<Tone> {
        let Some((value, block)) = self.roles_key("tones") else {
            return Vec::new();
        };
        let role_exists = |name: &str| roles.iter().any(|spec| spec.role.name == name);
        let mut tones = Vec::new();
        for (name, _) in entries(value) {
            if !ToneName::ALL.iter().any(|tone| tone.name() == name) {
                let line = Self::block_line(block, &[&name]);
                self.problem(
                    line,
                    format!("tones.{name}"),
                    "is not a tone: default, accent, ok, warning or error",
                );
            }
        }
        for tone in ToneName::ALL {
            let fields = &value[tone.name()];
            let line = Self::block_line(block, &[tone.name()]);
            let path = format!("tones.{}", tone.name());
            if fields.is_badvalue() {
                self.problem(line, &path, "this tone is missing");
                continue;
            }
            let role = |checker: &mut Self, field: &str, required: bool| -> Option<String> {
                match text(&fields[field]) {
                    Some(name) if role_exists(name) => Some(name.to_owned()),
                    Some(name) => {
                        checker.problem(
                            line,
                            format!("{path}.{field}"),
                            format!("{name} is not a role"),
                        );
                        None
                    }
                    None => {
                        if required {
                            checker.problem(line, format!("{path}.{field}"), "is required");
                        }
                        None
                    }
                }
            };
            let text_role = role(self, "text", true);
            let surface = role(self, "surface", true);
            let border = role(self, "border", false);
            let icon = role(self, "icon", false);
            let status_icon = text(&fields["status-icon"]).map(str::to_owned);
            if tone.is_status() {
                match &status_icon {
                    None => self.problem(
                        line,
                        format!("{path}.status-icon"),
                        "a status tone is never shown by color alone: it names an icon (FR-025)",
                    ),
                    Some(icon) if !icons.iter().any(|(name, _)| name == icon) => {
                        self.problem(
                            line,
                            format!("{path}.status-icon"),
                            format!("{icon} is not in icons"),
                        );
                    }
                    Some(_) => {}
                }
                if icon.is_none() {
                    self.problem(
                        line,
                        format!("{path}.icon"),
                        "a status tone names the role its icon is drawn in",
                    );
                }
            }
            if let (Some(text_role), Some(surface)) = (text_role, surface) {
                tones.push(Tone {
                    name: tone,
                    text: text_role,
                    surface,
                    border,
                    icon,
                    status_icon,
                });
            }
        }
        tones
    }

    fn elevation(&mut self) -> Vec<Elevation> {
        let Some((value, block)) = self.roles_key("elevation") else {
            return Vec::new();
        };
        let mut levels = Vec::new();
        for (name, fields) in entries(value) {
            let line = Self::block_line(block, &[&name]);
            let path = format!("elevation.{name}");
            let shadow = match text(&fields["shadow"]) {
                Some("none") => None,
                Some(written) => {
                    let parts: Vec<&str> = written.split_whitespace().collect();
                    let parsed = match parts.as_slice() {
                        [x, y, blur, color] => (|| {
                            let dim = |text: &str| text.strip_suffix("px")?.parse::<f64>().ok();
                            let step: u8 = color.strip_prefix("blackA-")?.parse().ok()?;
                            ((1..=STEPS as u8).contains(&step)).then_some(())?;
                            Some(Shadow {
                                x: dim(x)?,
                                y: dim(y)?,
                                blur: dim(blur)?,
                                black_a: step,
                            })
                        })(),
                        _ => None,
                    };
                    if parsed.is_none() {
                        self.problem(
                            line,
                            format!("{path}.shadow"),
                            "must be `none` or `<x>px <y>px <blur>px blackA-<1-12>`",
                        );
                    }
                    parsed
                }
                None => {
                    self.problem(line, format!("{path}.shadow"), "is required");
                    None
                }
            };
            let why = text(&fields["why"]).unwrap_or_default().trim().to_owned();
            if why.is_empty() {
                self.problem(line, format!("{path}.why"), "every level says why (FR-001)");
            }
            levels.push(Elevation {
                name,
                shadow,
                host_surface: fields["host-surface"].as_bool().unwrap_or(false),
                why,
            });
        }
        levels
    }

    fn radius_picks(&mut self, rounded: &[(String, f64)]) -> Vec<String> {
        let Some((value, block)) = self.roles_key("radius-picks") else {
            return Vec::new();
        };
        let names: Vec<String> = value
            .as_vec()
            .into_iter()
            .flatten()
            .filter_map(|n| n.as_str().map(str::to_owned))
            .collect();
        for name in &names {
            if !rounded.iter().any(|(token, _)| token == name) {
                let line = Self::block_line(block, &["radius-picks"]);
                self.problem(
                    line,
                    format!("radius-picks.{name}"),
                    "is not a rounded token",
                );
            }
        }
        names
    }

    // Rule 14 on icons: path data only, so nothing in DESIGN.md becomes markup in the preview.
    fn icons(&mut self) -> Vec<(String, String)> {
        let Some((value, block)) = self.roles_key("icons") else {
            return Vec::new();
        };
        let mut icons = Vec::new();
        for (name, path) in entries(value) {
            let data = text(path).unwrap_or_default();
            let clean = !data.is_empty()
                && data
                    .chars()
                    .all(|c| "MmLlHhVvCcSsQqTtAaZz0123456789 .,-".contains(c))
                && data.starts_with(['M', 'm']);
            if !clean {
                let line = Self::block_line(block, &[&name]);
                self.problem(
                    line,
                    format!("icons.{name}"),
                    "must be SVG path data only: commands and numbers",
                );
                continue;
            }
            icons.push((name, data.to_owned()));
        }
        icons
    }

    // Rule 6.
    fn rationale(&mut self, design: &Design) -> BTreeMap<String, String> {
        let Some((value, block)) = self.roles_key("rationale") else {
            return BTreeMap::new();
        };
        let mut rationale = BTreeMap::new();
        for (path, why) in entries(value) {
            rationale.insert(path, text(why).unwrap_or_default().trim().to_owned());
        }
        let mut wanted: Vec<String> = Vec::new();
        wanted.extend(
            design
                .typography
                .iter()
                .map(|(name, _)| format!("typography.{name}")),
        );
        wanted.extend(
            design
                .spacing
                .iter()
                .map(|(name, _)| format!("spacing.{name}")),
        );
        wanted.extend(
            design
                .rounded
                .iter()
                .map(|(name, _)| format!("rounded.{name}")),
        );
        wanted.extend(
            design
                .components
                .iter()
                .map(|component| format!("components.{}", component.name)),
        );
        for path in &wanted {
            if rationale.get(path).is_none_or(String::is_empty) {
                let segments: Vec<&str> = path.split('.').collect();
                let line = self.front_line(&segments);
                self.problem(
                    line,
                    path,
                    "has no rationale: add it under `rationale` in the project layer (FR-001)",
                );
            }
        }
        for path in rationale.keys() {
            if !wanted.contains(path) {
                let line = Self::block_line(block, &[path]);
                self.problem(
                    line,
                    format!("rationale.{path}"),
                    "names no token of the front matter",
                );
            }
        }
        rationale
    }

    // Rule 9.
    fn front_colors(&mut self, design: &Design) {
        let appearance = design.default_appearance();
        let colors = &self.source.front["colors"];
        let mut expected = BTreeMap::new();
        for spec in &design.roles {
            for (theme, key) in [
                (Theme::Light, spec.role.name.clone()),
                (Theme::Dark, format!("{}-dark", spec.role.name)),
            ] {
                match design.resolve(self.palette, &spec.role, theme, appearance) {
                    Ok(color) => {
                        expected.insert(key, color.to_hex());
                    }
                    Err(message) => {
                        let line = Self::block_line(
                            self.source.roles("roles").expect("read").1,
                            &[&spec.role.name],
                        );
                        self.problem(line, format!("roles.{}", spec.role.name), message);
                    }
                }
            }
        }
        for (key, value) in entries(colors) {
            let line = self.front_line(&["colors", &key]);
            let written = text(value).unwrap_or_default().to_ascii_lowercase();
            match expected.remove(&key) {
                None => self.problem(
                    line,
                    format!("colors.{key}"),
                    "is not a role, nor a role's -dark form",
                ),
                Some(color) if color != written => self.problem(
                    line,
                    format!("colors.{key}"),
                    format!(
                        "is {written}, but the role resolves to {color} at the default appearance"
                    ),
                ),
                Some(_) => {}
            }
        }
        for (key, color) in expected {
            let line = self.source.front_line_of("colors");
            self.problem(
                line,
                format!("colors.{key}"),
                format!("is missing: the role resolves to {color} at the default appearance"),
            );
        }
    }

    fn scale_kinds(&mut self, design: &Design) {
        let below = design.picks.neutral_below_chroma;
        for (index, tone) in ["ok", "warning", "error"].iter().enumerate() {
            if let Some(scale) = self.palette.scales.get(&design.status[index])
                && scale.kind(below) != Kind::Chromatic
            {
                self.problem(
                    1,
                    format!("scales.status.{tone}"),
                    "a status scale must be chromatic",
                );
            }
        }
    }
}

/// Checks a parsed `DESIGN.md` (rules 1–9, 12 and 14) and builds the `Design`.
pub fn check(source: &Source, palette: &Palette) -> Result<Design, Vec<Problem>> {
    let mut checker = Checker {
        source,
        palette,
        problems: Vec::new(),
    };
    let (name, description) = checker.format_layer();
    checker.project_keys();
    let typography = checker.typography();
    let spacing = checker.dimensions("spacing", true);
    let rounded = checker.dimensions("rounded", false);
    let (status, solid_text) = checker.scales();
    let roles = checker.roles();
    let host_roles: BTreeSet<String> = roles
        .iter()
        .filter(|spec| spec.role.host_surface)
        .map(|spec| spec.role.name.clone())
        .collect();
    let role_names: BTreeSet<String> = roles.iter().map(|spec| spec.role.name.clone()).collect();
    let components = checker.components(&typography, &role_names, &host_roles);
    let icons = checker.icons();
    let tones = checker.tones(&roles, &icons);
    let elevation = checker.elevation();
    let radius_picks = checker.radius_picks(&rounded);
    let picks = checker.picks(&radius_picks);
    let mut design = Design {
        name,
        description,
        status,
        solid_text,
        picks,
        roles,
        tones,
        elevation,
        radius_picks,
        icons,
        typography,
        spacing,
        rounded,
        components,
        rationale: BTreeMap::new(),
    };
    design.rationale = checker.rationale(&design);
    checker.scale_kinds(&design);
    if checker.problems.is_empty() {
        checker.front_colors(&design);
    }
    if checker.problems.is_empty() {
        let lint = crate::lint::lint(&design, palette, source);
        checker.problems.extend(lint);
    }
    let mut problems = checker.problems;
    problems.sort_by_key(|problem| problem.line);
    if problems.is_empty() {
        Ok(design)
    } else {
        Err(problems)
    }
}
