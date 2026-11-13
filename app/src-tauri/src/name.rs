//! The name a widget's frame draws (specs/009-declarative-widget-ui/research.md, R6; criterion
//! HS-02). The manifest refuses control characters and names over 60 characters when the plugin
//! loads; the runtime's sanitiser removes what could change how the name reads, and keeps it
//! short. It lives in the runtime so the installation dialog draws names and authors alike
//! (specs/010-plugin-installation/research.md, R6).
//!
//! Part of the security core (constitution, Principle IX).

pub use runtime::{DRAWN_NAME_CHARS, drawn_name};
