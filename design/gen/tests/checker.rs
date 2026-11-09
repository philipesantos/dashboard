//! The checker's rules (specs/007-host-visual-design/contracts/design-md.md, T012): the committed
//! DESIGN.md passes, and breaking it in one place for each rule is refused naming the place.

use crate::common::{design_md, mutate, palette, refused_naming};

#[test]
fn the_committed_design_md_passes() {
    if let Err(problems) = design_gen::check_text(&design_md(), &palette()) {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        panic!("DESIGN.md is refused:\n{}", lines.join("\n"));
    }
}

/// Rule 1: the front matter comes first, and the format's keys have the format's types.
#[test]
fn rule_1_front_matter() {
    let text = design_md();
    refused_naming(&text[4..], "front matter");
    refused_naming(&mutate(&text, "version: alpha", "version: 3"), "version");
    refused_naming(&mutate(&text, "fontSize: 14px", "fontSize: 14"), "fontSize");
    refused_naming(&mutate(&text, "description: ", "palette: x\ndescription: "), "palette");
}

/// Rule 2: every reference resolves to a primitive token, or a typography level in components.
#[test]
fn rule_2_references() {
    let text = design_md();
    refused_naming(&mutate(&text, "{rounded.medium}", "{rounded.huge}"), "rounded.huge");
    refused_naming(&mutate(&text, "\"{colors.text}\"", "\"{typography.body-md}\""), "typography.body-md");
}

/// Rule 3: the sections come in the format's order.
#[test]
fn rule_3_section_order() {
    let text = design_md()
        .replacen("\n## Colors\n", "\n## @@\n", 1)
        .replacen("\n## Layout\n", "\n## Colors\n", 1)
        .replacen("\n## @@\n", "\n## Layout\n", 1);
    refused_naming(&text, "order");
}

/// Rule 4: a design-roles block holds only the project layer's keys.
#[test]
fn rule_4_project_layer_keys() {
    let text = format!("{}\n```design-roles\nfonts: {{}}\n```\n", design_md());
    refused_naming(&text, "fonts");
}

/// Rule 5: a role names a step 1-12 (or white, black), a source that exists, and says why.
#[test]
fn rule_5_roles() {
    let text = design_md();
    refused_naming(&mutate(&text, "source: gray, light: 12, dark: 12", "source: gray, light: 13, dark: 12"), "text");
    refused_naming(&mutate(&text, "source: gray, light: 12, dark: 12", "source: grey, light: 12, dark: 12"), "grey");
    refused_naming(&mutate(&text, "why: \"Body text", "reason: \"Body text"), "why");
}

/// Rule 6: every format-layer token has a rationale.
#[test]
fn rule_6_rationale() {
    let text = design_md();
    refused_naming(&mutate(&text, "  spacing.md: ", "  spacing.mdx: "), "spacing.md");
}

/// Rule 7: a role is drawn only on roles that exist, and an exempt one says which exemption.
#[test]
fn rule_7_pairings() {
    let text = design_md();
    refused_naming(&mutate(&text, "on: [canvas, surface", "on: [nowhere, surface"), "nowhere");
    refused_naming(&mutate(&text, "exempt: \"", "note: \""), "exempt");
}

/// Rule 8: every tone has a text role, and each status tone a status icon from `icons`.
#[test]
fn rule_8_tones() {
    let text = design_md();
    refused_naming(&mutate(&text, "status-icon: check", "status-icon: tick"), "tick");
    refused_naming(&mutate(&text, "status-icon: alert", "symbol: alert"), "warning");
}

/// Rule 9: the front matter's colors are the roles resolved at the default appearance.
#[test]
fn rule_9_front_matter_colors() {
    let text = design_md();
    let line = text.lines().find(|line| line.starts_with("  text: \"#")).expect("colors.text");
    refused_naming(&mutate(&text, line, "  text: \"#123456\""), "colors.text");
}

/// Rule 12: a token that is not a host surface does not reference one that is.
#[test]
fn rule_12_host_surfaces_stay_apart() {
    let text = design_md();
    let frame = "  frame:\n    backgroundColor: \"{colors.surface}\"";
    refused_naming(
        &mutate(&text, frame, "  frame:\n    backgroundColor: \"{colors.dialog-surface}\""),
        "dialog-surface",
    );
}

/// Rule 14: no component takes an image, an icon source or a URL; icon paths are path data only.
#[test]
fn rule_14_no_images() {
    let text = design_md();
    let frame = "  frame:\n    backgroundColor: \"{colors.surface}\"";
    refused_naming(
        &mutate(&text, frame, &format!("{frame}\n    backgroundImage: \"https://example.com/x.png\"")),
        "backgroundImage",
    );
    refused_naming(&mutate(&text, "check: \"M", "check: \"<script/>M"), "check");
}
