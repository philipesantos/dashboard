//! Checks `DESIGN.md`, generates the design tokens, the DTCG export and the SVG preview, and runs
//! the contrast lint and the hue rule (specs/007-host-visual-design).

pub mod check;
pub mod color;
pub mod generate;
pub mod lint;
pub mod model;
pub mod preview;
pub mod radix;
pub mod source;

pub use check::{Design, Problem};

/// Parses and checks the text of a `DESIGN.md` against the palette.
pub fn check_text(text: &str, palette: &radix::Palette) -> Result<Design, Vec<Problem>> {
    let source = source::parse(text).map_err(|error| {
        vec![Problem {
            line: error.line,
            path: String::new(),
            message: error.message,
        }]
    })?;
    check::check(&source, palette)
}

/// Reads `DESIGN.md` and the vendored Radix files under `root` and checks them.
pub fn load(root: &std::path::Path) -> Result<(Design, radix::Palette), Vec<Problem>> {
    let fail = |message: String| {
        vec![Problem {
            line: 0,
            path: String::new(),
            message,
        }]
    };
    let palette =
        radix::read_vendored(&root.join("design")).map_err(|error| fail(error.to_string()))?;
    let text = std::fs::read_to_string(root.join("DESIGN.md"))
        .map_err(|error| fail(format!("DESIGN.md: {error}")))?;
    let design = check_text(&text, &palette)?;
    Ok((design, palette))
}
