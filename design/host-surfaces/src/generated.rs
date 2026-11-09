// Generated from DESIGN.md by `cargo xtask design`. Do not edit.

/// Which pick or fixed scale a role reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Gray,
    Widget,
    Host,
    Ok,
    Warning,
    Error,
    Overlay,
}

/// The step a role takes in one theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StepRef {
    Radix(u8),
    White,
    Black,
    SolidText,
}

/// A color role: where it reads its color in each theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RoleDef {
    pub(crate) name: &'static str,
    pub(crate) source: Source,
    pub(crate) light: StepRef,
    pub(crate) dark: StepRef,
}

pub(crate) const ROLES: &[RoleDef] = &[
    // roles.focus: The host's own focus ring; Radix's focus-ring steps 7-8 fail 3:1, and a widget-colored ring would vanish on lime or sky.
    RoleDef { name: "focus", source: Source::Host, light: StepRef::Radix(11), dark: StepRef::Radix(11) },
    // roles.chrome-surface: The host's own bar, in the host color when there is one; no frame or panel covers it.
    RoleDef { name: "chrome-surface", source: Source::Host, light: StepRef::Radix(3), dark: StepRef::Radix(3) },
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

    /// elevation.panel: A panel floats beside its frame, never over the chrome.
    pub const PANEL: Option<Shadow> = Some(Shadow { x: 0.0, y: 4.0, blur: 12.0, black_a: 4 });
    /// elevation.dialog: A host dialog floats above everything, over the scrim.
    pub const DIALOG: Option<Shadow> = Some(Shadow { x: 0.0, y: 12.0, blur: 32.0, black_a: 6 });
}

/// host-surfaces: the components only the host draws.
pub(crate) const COMPONENTS: [&str; 3] = ["chrome", "panel", "dialog"];
