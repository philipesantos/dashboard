//! The host's words (FR-039, research R7). Every string the host writes in its window comes from
//! this catalog, in English for now: a screen takes a [`HostText`], which only a [`Msg`] can
//! make, or a [`crate::label::PluginText`] for a plugin's name. `host/tests/checks.rs` fails on a
//! string literal handed to an element anywhere else.

use runtime::ErrorKind;

/// One thing the host says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Msg {
    // The chrome
    AppName,
    Reload,
    Appearance,
    // The empty dashboard
    NothingApproved,
    NothingApprovedHint,
    // A frame
    UnnamedWidget,
    WidgetMenu,
    MenuGlyph,
    WidgetSettings,
    Loading,
    Rendered,
    DidNotRender,
    // Why a widget did not render, by the runtime's error kind, never its detail
    ReasonPackage,
    ReasonUndeclared,
    ReasonApproval,
    ReasonLimits,
    ReasonCrashed,
    ReasonReported,
    ReasonOutput,
    ReasonConnector,
    ReasonStorage,
    ReasonAccount,
    ReasonUnreadable,
    // "Remove from dashboard"
    RemoveFromDashboard,
    RemoveDialogName,
    RemoveDialogQuestion,
    Cancel,
    Remove,
}

impl Msg {
    /// The English text.
    pub fn text(self) -> HostText {
        HostText(match self {
            Msg::AppName => "Dashboard",
            Msg::Reload => "Reload",
            Msg::Appearance => "Appearance",
            Msg::NothingApproved => "No widgets yet",
            Msg::NothingApprovedHint => {
                "Approve a plugin with the development tool, then choose Reload."
            }
            Msg::UnnamedWidget => "Unnamed widget",
            Msg::WidgetMenu => "Widget menu",
            Msg::MenuGlyph => "⋯",
            Msg::WidgetSettings => "Settings",
            Msg::Loading => "Loading",
            Msg::Rendered => "Rendered",
            Msg::DidNotRender => "Did not render",
            Msg::ReasonPackage => "The host cannot load this package.",
            Msg::ReasonUndeclared => "It asked for something its manifest does not declare.",
            Msg::ReasonApproval => {
                "Its approval does not cover this version. Approve it again with the development tool."
            }
            Msg::ReasonLimits => "It went over the host's limits.",
            Msg::ReasonCrashed => "It stopped with an error.",
            Msg::ReasonReported => "It reported an error.",
            Msg::ReasonOutput => "The host could not use what it returned.",
            Msg::ReasonConnector => "A connector it needs is not available in this window.",
            Msg::ReasonStorage => "Its storage could not be used.",
            Msg::ReasonAccount => "An account it needs is not available.",
            Msg::ReasonUnreadable => "Its package could not be read.",
            Msg::RemoveFromDashboard => "Remove from dashboard",
            Msg::RemoveDialogName => "Remove a widget",
            Msg::RemoveDialogQuestion => {
                "Remove this widget from the dashboard? Its permissions are withdrawn and the data the host kept for it is deleted. Its own storage stays."
            }
            Msg::Cancel => "Cancel",
            Msg::Remove => "Remove",
        })
    }

    /// The host's words for why a plugin did not load or render. Only the kind is used: the
    /// error's detail and names can quote the plugin (research R5).
    pub fn reason(kind: ErrorKind) -> Msg {
        match kind {
            ErrorKind::InvalidPackage
            | ErrorKind::ContractMismatch
            | ErrorKind::NoManifest
            | ErrorKind::ManifestMismatch
            | ErrorKind::InvalidManifest
            | ErrorKind::UnknownPermission
            | ErrorKind::OptionalNeedsV1
            | ErrorKind::ContractTooOld
            | ErrorKind::MissingUpdater
            | ErrorKind::InvalidConnector
            | ErrorKind::CredentialsNotSupported
            | ErrorKind::CredentialNeedsHttps
            | ErrorKind::AccountUseNotDeclared => Msg::ReasonPackage,
            ErrorKind::CapabilityRefused | ErrorKind::NotDeclared => Msg::ReasonUndeclared,
            ErrorKind::NotGranted
            | ErrorKind::NeedsNewApproval
            | ErrorKind::RequiredTogether
            | ErrorKind::ApprovalChanged => Msg::ReasonApproval,
            ErrorKind::CpuBudgetExceeded
            | ErrorKind::TimeLimitExceeded
            | ErrorKind::MemoryLimitExceeded => Msg::ReasonLimits,
            ErrorKind::PluginCrashed => Msg::ReasonCrashed,
            ErrorKind::PluginError => Msg::ReasonReported,
            ErrorKind::InvalidOutput | ErrorKind::OutputTooLarge => Msg::ReasonOutput,
            ErrorKind::DestinationNotVerified
            | ErrorKind::DestinationReserved
            | ErrorKind::ConnectorUnavailable => Msg::ReasonConnector,
            ErrorKind::StorageFailed => Msg::ReasonStorage,
            ErrorKind::SignInFailed
            | ErrorKind::AnotherAccountConnected
            | ErrorKind::KeyRefused
            | ErrorKind::KeychainUnavailable
            | ErrorKind::NoClientConfigured
            | ErrorKind::AccountNotConnected => Msg::ReasonAccount,
        }
    }
}

/// Text in the host's own words. Only [`Msg::text`] makes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HostText(&'static str);

impl HostText {
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl From<HostText> for gpui::SharedString {
    fn from(text: HostText) -> gpui::SharedString {
        gpui::SharedString::new_static(text.0)
    }
}
