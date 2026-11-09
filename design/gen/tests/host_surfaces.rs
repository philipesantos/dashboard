//! The host's surfaces stay out of the kit's reach (FR-014, FR-015, SC-003, T028): every token
//! DESIGN.md marks as a host surface is in host-surfaces and absent from design-tokens, nothing
//! else is in host-surfaces, and no other file includes their source.

use crate::common::{design_md, palette, root};

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).expect(path).replace("\r\n", "\n")
}

#[test]
fn every_host_surface_is_only_in_host_surfaces() {
    let design = design_gen::check_text(&design_md(), &palette()).expect("valid");
    let tokens = read("design/tokens/src/generated.rs");
    let host = read("design/host-surfaces/src/generated.rs");
    let mut marked = 0;
    for spec in &design.roles {
        let needle = format!("name: {:?},", spec.role.name);
        if spec.role.host_surface {
            marked += 1;
            assert!(host.contains(&needle), "{} is missing from host-surfaces", spec.role.name);
            assert!(!tokens.contains(&needle), "{} leaks into design-tokens", spec.role.name);
        } else {
            assert!(!host.contains(&needle), "{} is in host-surfaces but not marked", spec.role.name);
        }
    }
    for level in &design.elevation {
        let needle = format!("pub const {}: Option<Shadow>", level.name.to_ascii_uppercase());
        if level.host_surface {
            marked += 1;
            assert!(host.contains(&needle) && !tokens.contains(&needle), "elevation.{}", level.name);
        } else {
            assert!(tokens.contains(&needle) && !host.contains(&needle), "elevation.{}", level.name);
        }
    }
    assert!(marked >= 6, "chrome, dialog and panel surfaces, scrim, focus, and two levels: {marked}");
}

#[test]
fn no_file_includes_the_host_surfaces_source() {
    let root = root();
    let mut offenders = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("a directory") {
            let path = entry.expect("an entry").path();
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if path.is_dir() {
                if !["target", ".git", "node_modules"].contains(&name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") || path.starts_with(root.join("design/host-surfaces")) {
                continue;
            }
            // This test names the pattern itself
            if path.ends_with("design/gen/tests/host_surfaces.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let reaches = text.lines().any(|line| {
                (line.contains("include!") || line.contains("include_str!") || line.contains("#[path"))
                    && line.contains("host-surfaces/src")
            });
            if reaches {
                offenders.push(path.display().to_string());
            }
        }
    }
    assert!(offenders.is_empty(), "these files include host-surfaces' source: {offenders:?}");
}
