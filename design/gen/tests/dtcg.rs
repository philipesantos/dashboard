//! The W3C Design Tokens export (contracts/tokens-api.md, T014).

use crate::common::{design_md, palette};

fn export() -> String {
    let design = design_gen::check_text(&design_md(), &palette()).expect("valid");
    let generated = design_gen::generate::generate(&design, &palette());
    generated
        .files
        .iter()
        .find(|(path, _)| path == "design/tokens.dtcg.json")
        .map(|(_, text)| text.clone())
        .expect("the DTCG file")
}

#[test]
fn every_token_has_a_type_a_value_and_a_description() {
    let text = export();
    for group in [
        "\"color\"",
        "\"light\"",
        "\"dark\"",
        "\"typography\"",
        "\"spacing\"",
        "\"radius\"",
        "\"shadow\"",
    ] {
        assert!(text.contains(group), "the export has no {group} group");
    }
    let types = text.matches("\"$type\"").count();
    assert_eq!(
        types,
        text.matches("\"$value\"").count(),
        "every $type has a $value"
    );
    assert_eq!(
        types,
        text.matches("\"$description\"").count(),
        "every token has a $description"
    );
    assert!(types > 40, "only {types} tokens");
}

#[test]
fn host_surfaces_are_marked_and_only_they() {
    let text = export();
    let marked = text.matches("\"hostSurface\": true").count();
    let design = design_gen::check_text(&design_md(), &palette()).expect("valid");
    assert_eq!(
        marked,
        design.host_surface_count(),
        "every host surface is marked, nothing else"
    );
    assert!(marked >= 4, "chrome, dialog, scrim and focus at least");
}
