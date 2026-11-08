//! The strict reader of the vendored Radix files (specs/007-host-visual-design, T008, research R2):
//! it reads the 31 solid scales of each theme and `blackA`, skips the alpha and P3 exports it
//! recognises by name, and refuses anything else with the file and line.

use design_gen::radix::{SCALES, Theme, parse_exports, read};
use std::path::Path;

fn vendored(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../radix/3.0.0")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn block(name: &str, key: &str, steps: impl IntoIterator<Item = u8>) -> String {
    let mut text = format!("export const {name} = {{\n");
    for step in steps {
        text.push_str(&format!("  {key}{step}: \"#0090ff\",\n"));
    }
    text.push_str("};\n");
    text
}

#[test]
fn reads_every_scale_of_both_themes_and_black_a() {
    let palette = read(
        &vendored("light.ts"),
        &vendored("dark.ts"),
        &vendored("blackA.ts"),
    )
    .expect("the vendored files read");
    assert_eq!(SCALES.len(), 31);
    assert_eq!(palette.scales.len(), 31);
    for name in SCALES {
        let scale = &palette.scales[*name];
        assert_eq!(scale.step(Theme::Light, 1).to_hex().len(), 7, "{name}");
        assert_eq!(scale.step(Theme::Dark, 12).to_hex().len(), 7, "{name}");
    }
    assert_eq!(
        palette.scales["blue"].step(Theme::Light, 9).to_hex(),
        "#0090ff"
    );
    assert_eq!(
        palette.scales["gray"].step(Theme::Dark, 12).to_hex(),
        "#eeeeee"
    );
    assert_eq!(palette.black_a[6].a, 0.5, "blackA7");
    assert_eq!(palette.black_a[11].a, 0.95, "blackA12");
}

#[test]
fn a_well_formed_block_parses() {
    let exports = parse_exports("light.ts", &block("blue", "blue", 1..=12)).expect("parses");
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0].name, "blue");
    assert_eq!(exports[0].values.len(), 12);
}

#[test]
fn refuses_malformed_files_with_their_line() {
    let cases = [
        ("an unexpected line", "const x = 1;\n".to_owned(), 1),
        ("11 steps", block("blue", "blue", 1..=11), 1),
        ("13 steps", block("blue", "blue", 1..=13), 14),
        (
            "a step out of order",
            block("blue", "blue", [1, 2, 3, 5, 4, 6, 7, 8, 9, 10, 11, 12]),
            5,
        ),
        (
            "an unterminated block",
            "export const blue = {\n  blue1: \"#0090ff\",\n".to_owned(),
            1,
        ),
        (
            "a stray line inside a block",
            "export const blue = {\n  // a comment\n};\n".to_owned(),
            2,
        ),
    ];
    for (what, text, line) in cases {
        let error = parse_exports("light.ts", &text).expect_err(&format!("{what} must be refused"));
        assert_eq!(error.file, "light.ts", "{what}");
        assert_eq!(error.line, line, "{what}: {error}");
        assert!(
            error.to_string().starts_with(&format!("light.ts:{line}: ")),
            "{error}"
        );
    }
}

#[test]
fn refuses_a_missing_or_an_extra_scale() {
    let light = vendored("light.ts");
    let dark = vendored("dark.ts");
    let black_a = vendored("blackA.ts");
    // Drop the solid `blue` export of the light file
    let start = light.find("export const blue = {").expect("blue");
    let end = start + light[start..].find("};\n").expect("its end") + 3;
    let without_blue = format!("{}{}", &light[..start], &light[end..]);
    let error = read(&without_blue, &dark, &black_a).expect_err("blue is missing");
    assert!(error.to_string().contains("blue"), "{error}");

    let extra = format!("{light}{}", block("teal2", "teal2", 1..=12));
    let error = read(&extra, &dark, &black_a).expect_err("teal2 is not a Radix scale");
    assert!(error.to_string().contains("teal2"), "{error}");
}

#[test]
fn refuses_a_value_that_is_neither_hex_nor_rgba_in_a_solid_scale() {
    let light = vendored("light.ts").replacen("  blue9: \"#0090ff\",", "  blue9: \"blue\",", 1);
    let error = read(&light, &vendored("dark.ts"), &vendored("blackA.ts"))
        .expect_err("a named color is refused");
    assert!(error.to_string().contains("blue9"), "{error}");
}
