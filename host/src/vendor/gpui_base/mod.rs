//! Code adapted from gpui-base 0.9.0 (Apache-2.0), where its behavior is hard to get right: the
//! focus trap a host dialog keeps keyboard focus in (research R9). Its notice is in each file and
//! in LICENSE-APACHE; PROVENANCE.md records the source, its hashes and what changed.

pub mod focus_trap;
pub mod tab;

pub use focus_trap::FocusTrapElement;

/// Registers the focus trap manager and the Tab bindings; call before opening a window.
pub fn init(cx: &mut gpui::App) {
    focus_trap::init(cx);
    tab::init(cx);
}
