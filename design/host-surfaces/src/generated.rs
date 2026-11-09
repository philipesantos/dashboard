// Generated from DESIGN.md by `cargo xtask design`. Do not edit.

use design_tokens::raw::{RoleDef, Source, StepRef};

pub const ROLES: &[RoleDef] = &[
    // roles.focus: The host's own focus ring, in the host's ink: Radix's focus-ring steps 7-8 fail 3:1, and a widget-colored ring would vanish on lime or sky.
    RoleDef { name: "focus", source: Source::Host, light: StepRef::Radix(12), dark: StepRef::Radix(12) },
    // roles.host-strong: The host's ink: the fill of the monogram and of the actions the host draws in a frame's header. Step 12 of the host color, so it stays quiet when many widgets sit together.
    RoleDef { name: "host-strong", source: Source::Host, light: StepRef::Radix(12), dark: StepRef::Radix(12) },
    // roles.on-host-strong: Text on the host's ink: the monogram's letter and a header action's name.
    RoleDef { name: "on-host-strong", source: Source::Gray, light: StepRef::Radix(1), dark: StepRef::Radix(1) },
    // roles.chrome-surface: The host's own bar, in the host color when there is one; no frame or panel covers it.
    RoleDef { name: "chrome-surface", source: Source::Host, light: StepRef::Radix(4), dark: StepRef::Radix(2) },
    // roles.dialog-surface: A host dialog's surface; what tells a dialog apart is its form, not this color.
    RoleDef { name: "dialog-surface", source: Source::Gray, light: StepRef::Radix(1), dark: StepRef::Radix(2) },
    // roles.panel-surface: The panel a frame opens for a plugin's controls, attached to that frame.
    RoleDef { name: "panel-surface", source: Source::Gray, light: StepRef::Radix(1), dark: StepRef::Radix(2) },
    // roles.scrim: Dims the window behind a host dialog; blackA because gray 12 is near white in the dark theme.
    RoleDef { name: "scrim", source: Source::Overlay, light: StepRef::Radix(7), dark: StepRef::Radix(9) },
];

/// Elevation levels only the host draws.
pub mod elevation {
/// A shadow: offsets and blur in pixels, its color a step of Radix's blackA.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Shadow {
        pub x: f32,
        pub y: f32,
        pub blur: f32,
        pub black_a: u8,
    }

    /// elevation.panel: A panel stays attached to its frame and is told apart by its line, never by floating.
    pub const PANEL: Option<Shadow> = None;
    /// elevation.dialog: A host dialog stands out by its form: the scrim under it covers the whole window, chrome included.
    pub const DIALOG: Option<Shadow> = None;
}

/// host-surfaces: the components only the host draws.
pub const COMPONENTS: [&str; 5] = ["chrome", "monogram", "frame-action", "panel", "dialog"];
