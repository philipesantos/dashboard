//! The widget kit: what a plugin's `render` output can ask the host to draw, and nothing more.
//!
//! A plugin returns JSON (`wit/v1`, `renderer.render`). [`model::Content::parse`] reads it into
//! a closed set of blocks (text, list, table, button) under fixed limits, refusing anything
//! else, and [`render::content`] draws those blocks with the host's components, built on
//! gpui-base, with the widget tokens of [`tokens::WidgetTheme`] only. The host's own tokens
//! (frame, dialogs, the visual secret) live in the host crate, which this crate cannot see.

pub mod model;
pub mod render;
pub mod tokens;
