//! Checks `DESIGN.md`, generates the design tokens, the DTCG export and the SVG preview, and runs
//! the contrast lint and the hue rule (specs/007-host-visual-design).

pub mod check;
pub mod color;
pub mod model;
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
