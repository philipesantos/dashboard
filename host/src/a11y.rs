//! What GPUI has no builder for (research R10): marking a host dialog modal, and hiding every
//! host window's background from assistive technology while a dialog is open. AccessKit's shared
//! filter, which every platform adapter uses, leaves a hidden node and its subtree out of the
//! tree; `modal` is what Orca, UI Automation and macOS read as a modal dialog (HS-18, HS-19).

use gpui::StatefulInteractiveElement;

pub trait Modality: StatefulInteractiveElement + Sized {
    /// Marks this element's node modal. It needs an id and a role, as every node does.
    fn modal(self) -> Self {
        self.a11y_synthetic_children(|builder| {
            builder.parent_node().set_modal();
        })
    }

    /// Hides this element's node and its subtree from assistive technology while `hidden`.
    fn hidden_while(self, hidden: bool) -> Self {
        if hidden {
            self.a11y_synthetic_children(|builder| {
                builder.parent_node().set_hidden();
            })
        } else {
            self
        }
    }
}

impl<E: StatefulInteractiveElement + Sized> Modality for E {}
