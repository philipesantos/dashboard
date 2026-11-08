# Contract: the token crates and `cargo xtask design`

What later Phase 3 features build against. Names are the contract; bodies are generated or
written during implementation.

## `design-tokens` (crate `design/tokens`)

For the kit and the host. Never holds a host-surface token (R6).

```rust
pub enum Theme { Light, Dark }
pub enum ThemePick { Light, Dark, System }
pub enum GrayFamily { Gray, Mauve, Slate, Sage, Olive, Sand }
pub enum Scale { /* the 25 chromatic and neutral scales, generated */ }
pub enum HostColor { Monochrome, Scale(Scale) }
pub enum Radius { None, Small, Medium, Large, Full }

pub struct Appearance { pub gray: GrayFamily, pub host: HostColor, pub widget: Scale,
                        pub radius: Radius, pub theme: ThemePick }
impl Appearance {
    pub const DEFAULT: Appearance;
    pub fn validate(&self) -> Result<(), Refusal>;
}
pub enum Refusal {
    TooClose { widget: Scale, other: Other, degrees: u16 },   // Other: the host color or a status
    NotChromatic { scale: Scale },
}

pub enum Tone { Default, Accent, Ok, Warning, Error }
impl Tone { pub fn parse(name: &str) -> Result<Tone, UnknownTone>; }
pub struct UnknownTone(pub String);   // the kit draws Default and logs this name escaped (FR-017)
pub struct ToneColors { pub text: Rgb, pub surface: Rgb, pub border: Rgb, pub icon: Option<Icon> }
pub fn tone(tone: Tone, appearance: &Appearance, theme: Theme) -> ToneColors;

pub struct Rgb { pub r: u8, pub g: u8, pub b: u8 }
pub fn contrast(a: Rgb, b: Rgb) -> f64;                      // WCAG 2.2 ratio

pub mod typography { /* generated levels */ }
pub mod spacing { /* generated steps */ }
pub mod elevation { /* flat, frame */ }
pub enum Icon { Check, Alert, Cross }
```

No item of this crate is a host-surface token; a `design-gen` test checks every marked token
against the generated files (R6, SC-003).

## `host-surfaces` (crate `design/host-surfaces`)

For the host only; `deny.toml` bans it except for the crate `host`, and bans `design-gen` except
for `xtask` (R6).

```rust
pub fn chrome(appearance: &Appearance, theme: Theme) -> ChromeColors;
pub fn dialog(appearance: &Appearance, theme: Theme) -> DialogStyle;   // surface, scrim, elevation
pub fn panel_attachment(appearance: &Appearance, theme: Theme) -> PanelStyle;
pub fn focus(appearance: &Appearance, theme: Theme) -> FocusStyle;
```

## `cargo xtask design`

| Command | Does | Exit |
| --- | --- | --- |
| `cargo xtask design` | checks `DESIGN.md`, then writes `design/tokens/src/generated.rs`, `design/host-surfaces/src/generated.rs` and `design/tokens.dtcg.json` | 0, or 1 with the first failure |
| `cargo xtask design --check` | runs every check and compares the generated files, writing nothing | 0, or 1 naming the token, pairing or line |
| `cargo xtask design --preview <dir>` | writes the SVG preview files of R8 into `<dir>` | 0, or 1 |

A failure prints one line per problem, with the file and line or the token path, the theme and
the picks for a contrast failure, and the measured ratio, escaped as the CLI escapes text.

## `design/tokens.dtcg.json`

W3C Design Tokens Format Module, current draft: `$type` and `$value` per token, groups for
`color.light`, `color.dark`, `typography`, `spacing`, `radius`, `shadow`; `$description` carries
the rationale; `$extensions."dev.dashboard".hostSurface: true` marks host surfaces.
