//! Reads `DESIGN.md` into its two layers (specs/007-host-visual-design/contracts/design-md.md):
//! the front matter of the DESIGN.md format, the `##` sections in order, and every fenced
//! `design-roles` block as YAML. It only parses; the rules are in `check`.

use std::fmt;
use yaml_rust2::{Yaml, YamlLoader};

/// The info string of the project layer's fenced blocks.
pub const ROLES_INFO: &str = "design-roles";

/// A refusal, with the line of `DESIGN.md` it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "DESIGN.md:{}: {}", self.line, self.message)
    }
}

impl std::error::Error for SourceError {}

fn error(line: usize, message: impl Into<String>) -> SourceError {
    SourceError {
        line,
        message: message.into(),
    }
}

/// A `##` heading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub line: usize,
}

/// A fenced `design-roles` block.
#[derive(Clone, Debug)]
pub struct Block {
    /// The `##` section it sits in, if any.
    pub section: Option<String>,
    /// The line of its opening fence.
    pub line: usize,
    /// Its YAML, always a mapping.
    pub yaml: Yaml,
    /// Its lines, without the fences, to find where a key is written.
    pub lines: Vec<String>,
}

impl Block {
    /// The line of `DESIGN.md` where `key:` first appears in this block, or the fence's line.
    pub fn line_of(&self, key: &str) -> usize {
        let wanted = format!("{key}:");
        self.lines
            .iter()
            .position(|line| {
                let trimmed = line
                    .trim_start()
                    .trim_start_matches("- ")
                    .trim_start_matches('{');
                trimmed.starts_with(&wanted) || line.contains(&format!(" {wanted}"))
            })
            .map_or(self.line, |index| self.line + 1 + index)
    }
}

/// `DESIGN.md`, parsed.
#[derive(Clone, Debug)]
pub struct Source {
    /// The front matter, always a mapping.
    pub front: Yaml,
    /// The front matter's lines, between the `---` lines (which are lines 1 and
    /// `front_lines.len() + 2`).
    pub front_lines: Vec<String>,
    pub sections: Vec<Section>,
    pub blocks: Vec<Block>,
}

impl Source {
    /// The line where `key:` first appears in the front matter, or line 1.
    pub fn front_line_of(&self, key: &str) -> usize {
        let wanted = format!("{key}:");
        self.front_lines
            .iter()
            .position(|line| line.trim_start().starts_with(&wanted))
            .map_or(1, |index| index + 2)
    }

    /// The `design-roles` value under a top-level key, and the block it is in.
    pub fn roles(&self, key: &str) -> Option<(&Yaml, &Block)> {
        self.blocks.iter().find_map(|block| {
            let value = &block.yaml[key];
            (!value.is_badvalue()).then_some((value, block))
        })
    }
}

/// The format's section names, with their aliases, in the order they must appear.
pub const SECTION_ORDER: &[&[&str]] = &[
    &["Overview", "Brand & Style"],
    &["Colors"],
    &["Typography"],
    &["Layout", "Layout & Spacing"],
    &["Elevation & Depth", "Elevation"],
    &["Shapes"],
    &["Components"],
    &["Do's and Don'ts"],
];

/// The position of a section in the format's order, or `None` for a section the format does not
/// name (which it keeps).
pub fn canonical_position(title: &str) -> Option<usize> {
    SECTION_ORDER.iter().position(|names| {
        names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(title.trim()))
    })
}

fn load(text: &str, first_line: usize, what: &str) -> Result<Yaml, SourceError> {
    let mut documents = YamlLoader::load_from_str(text).map_err(|scan| {
        error(
            first_line + scan.marker().line().saturating_sub(1),
            format!("{what}: {scan}"),
        )
    })?;
    match documents.len() {
        0 => Ok(Yaml::Hash(Default::default())),
        1 => Ok(documents.remove(0)),
        _ => Err(error(
            first_line,
            format!("{what} holds more than one YAML document"),
        )),
    }
}

/// The number of backticks a fence line opens or closes with, if it is one.
fn fence(line: &str) -> Option<usize> {
    let ticks = line.bytes().take_while(|byte| *byte == b'`').count();
    (ticks >= 3).then_some(ticks)
}

/// Parses `DESIGN.md`.
pub fn parse(text: &str) -> Result<Source, SourceError> {
    let lines: Vec<&str> = text
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    if lines.first() != Some(&"---") {
        return Err(error(
            1,
            "the front matter must come first, opened by a line `---`",
        ));
    }
    let close = lines
        .iter()
        .skip(1)
        .position(|line| *line == "---")
        .map(|index| index + 1)
        .ok_or_else(|| error(1, "the front matter is not closed by a line `---`"))?;
    let front_lines: Vec<String> = lines[1..close]
        .iter()
        .map(|line| (*line).to_owned())
        .collect();
    let front = load(&front_lines.join("\n"), 2, "the front matter")?;
    if front.as_hash().is_none() {
        return Err(error(2, "the front matter must be a YAML mapping"));
    }

    let mut sections: Vec<Section> = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut index = close + 1;
    while index < lines.len() {
        let number = index + 1;
        let line = lines[index];
        if let Some(ticks) = fence(line) {
            let info = line[ticks..].trim();
            let end = lines[index + 1..]
                .iter()
                .position(|candidate| {
                    fence(candidate).is_some_and(|n| n >= ticks)
                        && candidate.trim_start_matches('`').trim().is_empty()
                })
                .map(|offset| index + 1 + offset)
                .ok_or_else(|| error(number, "this code block is not closed"))?;
            if info == ROLES_INFO {
                let body: Vec<String> = lines[index + 1..end]
                    .iter()
                    .map(|l| (*l).to_owned())
                    .collect();
                let yaml = load(&body.join("\n"), number + 1, "this design-roles block")?;
                if yaml.as_hash().is_none() {
                    return Err(error(number, "a design-roles block must be a YAML mapping"));
                }
                blocks.push(Block {
                    section: sections.last().map(|section| section.title.clone()),
                    line: number,
                    yaml,
                    lines: body,
                });
            }
            index = end + 1;
            continue;
        }
        if let Some(title) = line.strip_prefix("## ") {
            let title = title.trim().to_owned();
            let same = |other: &Section| match (
                canonical_position(&other.title),
                canonical_position(&title),
            ) {
                (Some(a), Some(b)) => a == b,
                _ => other.title.eq_ignore_ascii_case(&title),
            };
            if let Some(first) = sections.iter().find(|other| same(other)) {
                return Err(error(
                    number,
                    format!(
                        "the section {title:?} appears twice (first at line {})",
                        first.line
                    ),
                ));
            }
            sections.push(Section {
                title,
                line: number,
            });
        }
        index += 1;
    }

    // Each top-level key of the project layer is defined in exactly one block
    let mut seen: Vec<(String, usize)> = Vec::new();
    for block in &blocks {
        for key in block.yaml.as_hash().expect("checked above").keys() {
            let name = key
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{key:?}"));
            let line = block.line_of(&name);
            if let Some((_, first)) = seen.iter().find(|(seen, _)| *seen == name) {
                return Err(error(
                    line,
                    format!(
                        "the design-roles key {name:?} is defined twice (first at line {first})"
                    ),
                ));
            }
            seen.push((name, line));
        }
    }

    Ok(Source {
        front,
        front_lines,
        sections,
        blocks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = "---\nversion: alpha\nname: Test\n---\n\n## Overview\n\nText.\n\n## Colors\n\n```design-roles\nroles:\n  text: { source: gray, light: 12, dark: 12, why: \"Body text.\" }\n```\n";

    #[test]
    fn parses_both_layers() {
        let source = parse(MINIMAL).expect("parses");
        assert_eq!(source.front["name"].as_str(), Some("Test"));
        assert_eq!(source.sections.len(), 2);
        assert_eq!(
            source.sections[1],
            Section {
                title: "Colors".into(),
                line: 10
            }
        );
        assert_eq!(source.blocks.len(), 1);
        assert_eq!(source.blocks[0].section.as_deref(), Some("Colors"));
        assert_eq!(source.blocks[0].line, 12);
        let (roles, block) = source.roles("roles").expect("roles");
        assert_eq!(roles["text"]["light"].as_i64(), Some(12));
        assert_eq!(block.line_of("text"), 14);
        assert_eq!(source.front_line_of("name"), 3);
    }

    #[test]
    fn front_matter_must_come_first() {
        let error = parse(&format!("\n{MINIMAL}")).expect_err("a blank line first");
        assert_eq!(error.line, 1);
        let error = parse("---\nname: Test\n").expect_err("never closed");
        assert_eq!(error.line, 1);
    }

    #[test]
    fn a_section_appears_once() {
        let text = format!("{MINIMAL}\n## Colors\n");
        let error = parse(&text).expect_err("two Colors");
        assert_eq!(error.line, 17, "{error}");
        // An alias counts as the same section
        let text = format!("{MINIMAL}\n## Brand & Style\n");
        assert!(parse(&text).is_err(), "Brand & Style is Overview");
    }

    #[test]
    fn a_roles_key_is_defined_once() {
        let text = format!("{MINIMAL}\n## Shapes\n\n```design-roles\nroles:\n  other: {{}}\n```\n");
        let error = parse(&text).expect_err("roles twice");
        assert_eq!(error.line, 20, "{error}");
        assert!(error.message.contains("roles"), "{error}");
    }

    #[test]
    fn yaml_that_does_not_parse_is_refused_with_its_line() {
        let text = "---\nname: [unclosed\n---\n";
        let error = parse(text).expect_err("bad front matter");
        assert!(error.line >= 2, "{error}");
        let text = format!("{MINIMAL}\n```design-roles\nicons: {{ check: \"M0\"\n```\n");
        let error = parse(&text).expect_err("bad block");
        assert!(error.line >= 18, "{error}");
    }

    #[test]
    fn other_code_blocks_are_not_read() {
        let text = format!("{MINIMAL}\n```yaml\ncolors: 3\n```\n\n```\n## not a section\n```\n");
        let source = parse(&text).expect("parses");
        assert_eq!(source.blocks.len(), 1);
        assert_eq!(source.sections.len(), 2);
    }
}
