//! The SVG preview (research R8, T015): five files, no script, nothing outside the directory but
//! the embedded font.

use crate::common::{design_md, palette, root};

#[test]
fn five_static_svg_files_with_the_font_embedded() {
    let design = design_gen::check_text(&design_md(), &palette()).expect("valid");
    let files =
        design_gen::preview::render(&design, &palette(), &root().join("design")).expect("renders");
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "index.svg",
            "palette-light.svg",
            "palette-dark.svg",
            "default-light.svg",
            "default-dark.svg"
        ]
    );
    for (name, svg) in &files {
        assert!(
            svg.starts_with("<svg ") || svg.starts_with("<?xml"),
            "{name}"
        );
        assert!(svg.trim_end().ends_with("</svg>"), "{name}");
        let lower = svg.to_ascii_lowercase();
        assert!(!lower.contains("<script"), "{name} has a script");
        assert!(
            !lower.contains(" on") || !regex_like_handler(&lower),
            "{name} has an event handler"
        );
        for (at, _) in lower.match_indices("href=\"") {
            let target = &lower[at + 6
                ..lower[at + 6..]
                    .find('"')
                    .map_or(lower.len(), |end| at + 6 + end)];
            assert!(
                target.ends_with(".svg") && !target.contains('/') && !target.contains(':'),
                "{name} links outside the directory: {target}"
            );
        }
        for (at, _) in lower.match_indices("url(") {
            assert!(
                lower[at..].starts_with("url(data:font/woff2;base64,"),
                "{name} has url() that is not the font"
            );
        }
        if name != "index.svg" {
            assert!(
                svg.contains("font/woff2;base64,"),
                "{name} does not embed the host's fonts"
            );
        }
    }
}

/// An attribute like ` onload=` or ` onclick=`.
fn regex_like_handler(lower: &str) -> bool {
    lower.match_indices(" on").any(|(at, _)| {
        let rest = &lower[at + 3..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        !name.is_empty() && rest[name.len()..].starts_with('=')
    })
}
