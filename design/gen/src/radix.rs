//! A strict reader of the vendored Radix Colors files, `design/radix/3.0.0/{light,dark,blackA}.ts`
//! (specs/007-host-visual-design/research.md, R2). It is not a TypeScript parser: it accepts only
//! the lines Radix writes, takes the solid scales of each theme and `blackA`, skips the alpha and
//! P3 exports it recognises by name, and refuses anything else with the file and line.

use crate::color::{Rgb, Rgba, oklch};
use std::collections::BTreeMap;
use std::fmt;

/// The six gray families, in Radix's order.
pub const GRAY_FAMILIES: &[&str] = &["gray", "mauve", "slate", "sage", "olive", "sand"];

/// Every solid Radix scale, gray families first, in the order the files export them.
pub const SCALES: &[&str] = &[
    "gray", "mauve", "slate", "sage", "olive", "sand", "tomato", "red", "ruby", "crimson", "pink",
    "plum", "purple", "violet", "iris", "indigo", "blue", "cyan", "teal", "jade", "green", "grass",
    "brown", "bronze", "gold", "sky", "mint", "lime", "yellow", "amber", "orange",
];

/// Steps per scale.
pub const STEPS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub const BOTH: [Theme; 2] = [Theme::Light, Theme::Dark];

    pub fn name(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}

/// What a scale can be picked as (data-model.md, Scale).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    GrayFamily,
    /// Chroma under the threshold: never a host or widget color (FR-013).
    Neutral,
    Chromatic,
}

/// One solid scale, both themes.
#[derive(Clone, Debug, PartialEq)]
pub struct Scale {
    pub name: String,
    pub light: [Rgb; STEPS],
    pub dark: [Rgb; STEPS],
}

impl Scale {
    /// Step 1–12 of the theme.
    pub fn step(&self, theme: Theme, step: u8) -> Rgb {
        assert!(
            (1..=STEPS as u8).contains(&step),
            "Radix steps are 1-12, not {step}"
        );
        let steps = match theme {
            Theme::Light => &self.light,
            Theme::Dark => &self.dark,
        };
        steps[usize::from(step) - 1]
    }

    /// OKLCH hue of light step 9, in degrees (research R3).
    pub fn hue(&self) -> f64 {
        oklch(self.step(Theme::Light, 9)).h
    }

    /// OKLCH chroma of light step 9 (research R3).
    pub fn chroma(&self) -> f64 {
        oklch(self.step(Theme::Light, 9)).c
    }

    pub fn kind(&self, neutral_below_chroma: f64) -> Kind {
        if GRAY_FAMILIES.contains(&self.name.as_str()) {
            Kind::GrayFamily
        } else if self.chroma() < neutral_below_chroma {
            Kind::Neutral
        } else {
            Kind::Chromatic
        }
    }
}

/// Everything the generator takes from Radix.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub scales: BTreeMap<String, Scale>,
    /// `blackA1`–`blackA12`, the overlay scale the scrim and shadows use in both themes.
    pub black_a: [Rgba; STEPS],
}

/// A refusal, with the file and the line (0 when it is about the whole file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadError {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(formatter, "{}: {}", self.file, self.message)
        } else {
            write!(formatter, "{}:{}: {}", self.file, self.line, self.message)
        }
    }
}

impl std::error::Error for ReadError {}

fn error(file: &str, line: usize, message: impl Into<String>) -> ReadError {
    ReadError {
        file: file.to_owned(),
        line,
        message: message.into(),
    }
}

/// One `key: "value",` line of an export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: String,
    pub line: usize,
}

/// One `export const <name> = { … };` block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub line: usize,
    pub values: Vec<Entry>,
}

fn is_name(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// Parses a file into its export blocks, checking only their shape: each block has exactly 12
/// entries whose keys end in the steps 1 to 12, in order, each with a quoted value.
pub fn parse_exports(file: &str, text: &str) -> Result<Vec<Export>, ReadError> {
    let mut exports = Vec::new();
    let mut open: Option<Export> = None;
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        match open.as_mut() {
            None => {
                if line.is_empty() {
                    continue;
                }
                let name = line
                    .strip_prefix("export const ")
                    .and_then(|rest| rest.strip_suffix(" = {"))
                    .filter(|name| is_name(name))
                    .ok_or_else(|| error(file, number, format!("unexpected line {line:?}")))?;
                open = Some(Export {
                    name: name.to_owned(),
                    line: number,
                    values: Vec::new(),
                });
            }
            Some(export) => {
                if line == "};" {
                    if export.values.len() != STEPS {
                        return Err(error(
                            file,
                            export.line,
                            format!(
                                "{} has {} steps, not {STEPS}",
                                export.name,
                                export.values.len()
                            ),
                        ));
                    }
                    exports.push(open.take().expect("an open block"));
                    continue;
                }
                let entry = line
                    .strip_prefix("  ")
                    .and_then(|rest| rest.split_once(": \""))
                    .and_then(|(key, rest)| Some((key, rest.strip_suffix("\",")?)))
                    .filter(|(key, value)| is_name(key) && !value.contains('"'))
                    .ok_or_else(|| error(file, number, format!("unexpected line {line:?}")))?;
                let (key, value) = entry;
                let digits = key.trim_start_matches(|c: char| !c.is_ascii_digit());
                let step: usize = digits.parse().unwrap_or(0);
                let expected = export.values.len() + 1;
                if expected > STEPS {
                    return Err(error(
                        file,
                        number,
                        format!("{} has a step {expected}", export.name),
                    ));
                }
                if step != expected || key.len() == digits.len() {
                    return Err(error(
                        file,
                        number,
                        format!("{key}: step {expected} was expected here"),
                    ));
                }
                export.values.push(Entry {
                    key: key.to_owned(),
                    value: value.to_owned(),
                    line: number,
                });
            }
        }
    }
    if let Some(export) = open {
        return Err(error(
            file,
            export.line,
            format!("{} is not closed with `}};`", export.name),
        ));
    }
    Ok(exports)
}

/// What a file's export is to the reader.
enum Role {
    Take(String),
    Skip,
}

fn classify(file: &str, export: &Export, suffix: &str) -> Result<Role, ReadError> {
    let name = export.name.as_str();
    for scale in SCALES {
        let base = format!("{scale}{suffix}");
        if name == base {
            return Ok(Role::Take((*scale).to_owned()));
        }
        if [
            format!("{base}A"),
            format!("{base}P3"),
            format!("{base}P3A"),
        ]
        .iter()
        .any(|n| n == name)
        {
            return Ok(Role::Skip);
        }
    }
    Err(error(
        file,
        export.line,
        format!("export {name} is not a Radix scale this reader knows"),
    ))
}

fn solid(file: &str, export: &Export) -> Result<[Rgb; STEPS], ReadError> {
    let mut steps = [Rgb { r: 0, g: 0, b: 0 }; STEPS];
    for (slot, entry) in steps.iter_mut().zip(&export.values) {
        *slot = Rgb::parse_hex(&entry.value).ok_or_else(|| {
            error(
                file,
                entry.line,
                format!("{}: {:?} is not #rrggbb", entry.key, entry.value),
            )
        })?;
    }
    Ok(steps)
}

fn theme(
    file: &str,
    text: &str,
    suffix: &str,
) -> Result<BTreeMap<String, [Rgb; STEPS]>, ReadError> {
    let mut taken = BTreeMap::new();
    for export in parse_exports(file, text)? {
        if let Role::Take(scale) = classify(file, &export, suffix)?
            && taken.insert(scale, solid(file, &export)?).is_some()
        {
            return Err(error(
                file,
                export.line,
                format!("{} is exported twice", export.name),
            ));
        }
    }
    for scale in SCALES {
        if !taken.contains_key(*scale) {
            return Err(error(
                file,
                0,
                format!("the scale {scale}{suffix} is missing"),
            ));
        }
    }
    Ok(taken)
}

/// Reads the three vendored files.
pub fn read(light: &str, dark: &str, black_a: &str) -> Result<Palette, ReadError> {
    let mut light = theme("light.ts", light, "")?;
    let mut dark = theme("dark.ts", dark, "Dark")?;
    let mut overlay = None;
    for export in parse_exports("blackA.ts", black_a)? {
        match export.name.as_str() {
            "blackA" => {
                let mut steps = [Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0.0,
                }; STEPS];
                for (slot, entry) in steps.iter_mut().zip(&export.values) {
                    *slot = Rgba::parse_rgba(&entry.value).ok_or_else(|| {
                        error(
                            "blackA.ts",
                            entry.line,
                            format!("{}: {:?} is not rgba(r, g, b, a)", entry.key, entry.value),
                        )
                    })?;
                }
                overlay = Some(steps);
            }
            "blackP3A" => {}
            other => {
                return Err(error(
                    "blackA.ts",
                    export.line,
                    format!("export {other} is not one this reader knows"),
                ));
            }
        }
    }
    let black_a = overlay.ok_or_else(|| error("blackA.ts", 0, "the scale blackA is missing"))?;
    let scales = SCALES
        .iter()
        .map(|name| {
            let scale = Scale {
                name: (*name).to_owned(),
                light: light.remove(*name).expect("checked above"),
                dark: dark.remove(*name).expect("checked above"),
            };
            ((*name).to_owned(), scale)
        })
        .collect();
    Ok(Palette { scales, black_a })
}

/// Reads `design/radix/3.0.0/` from the repository.
pub fn read_vendored(design_dir: &std::path::Path) -> Result<Palette, Box<dyn std::error::Error>> {
    let dir = design_dir.join("radix").join("3.0.0");
    let file = |name: &str| std::fs::read_to_string(dir.join(name));
    Ok(read(
        &file("light.ts")?,
        &file("dark.ts")?,
        &file("blackA.ts")?,
    )?)
}
