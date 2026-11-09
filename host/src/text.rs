//! The host's words (FR-039, research R7). Every string the host writes in its window comes from
//! this catalog, in English for now: a screen takes a [`HostText`], which only a [`Msg`] can
//! make, or a [`crate::label::PluginText`] for a plugin's name. `host/tests/checks.rs` fails on a
//! string literal handed to an element anywhere else.

use design_tokens::{GrayFamily, HUE_DISTANCE, Other, Refusal, Scale};
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
    PanelEmpty,
    Close,
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
    // The appearance settings
    AppearanceHint,
    AppearanceDefaulted,
    AppearanceNotSaved,
    GroupGray,
    GroupHost,
    GroupWidget,
    GroupCorners,
    GroupTheme,
    Monochrome,
    ThemeLight,
    ThemeDark,
    ThemeSystem,
    RadiusNone,
    RadiusSmall,
    RadiusMedium,
    RadiusLarge,
    RadiusFull,
    StatusOk,
    StatusWarning,
    StatusError,
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
            Msg::PanelEmpty => "The settings this widget declares will appear here.",
            Msg::Close => "Close",
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
            Msg::AppearanceHint => {
                "The host color marks what only the host draws. Widgets draw in the widget color, which stays apart from the host color and the status colors."
            }
            Msg::AppearanceDefaulted => {
                "The saved appearance could not be used, so the default is shown. Pick it again in Appearance."
            }
            Msg::AppearanceNotSaved => {
                "This appearance applies now, but it could not be saved for the next time."
            }
            Msg::GroupGray => "Gray",
            Msg::GroupHost => "Host color",
            Msg::GroupWidget => "Widget color",
            Msg::GroupCorners => "Corners",
            Msg::GroupTheme => "Theme",
            Msg::Monochrome => "Monochrome",
            Msg::ThemeLight => "Light",
            Msg::ThemeDark => "Dark",
            Msg::ThemeSystem => "Match the system",
            Msg::RadiusNone => "Square",
            Msg::RadiusSmall => "Small",
            Msg::RadiusMedium => "Medium",
            Msg::RadiusLarge => "Large",
            Msg::RadiusFull => "Round",
            Msg::StatusOk => "ok",
            Msg::StatusWarning => "warning",
            Msg::StatusError => "error",
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

/// The host's name for a color scale of `DESIGN.md`.
pub fn scale_name(scale: Scale) -> HostText {
    HostText(match scale {
        Scale::Tomato => "Tomato",
        Scale::Red => "Red",
        Scale::Ruby => "Ruby",
        Scale::Crimson => "Crimson",
        Scale::Pink => "Pink",
        Scale::Plum => "Plum",
        Scale::Purple => "Purple",
        Scale::Violet => "Violet",
        Scale::Iris => "Iris",
        Scale::Indigo => "Indigo",
        Scale::Blue => "Blue",
        Scale::Cyan => "Cyan",
        Scale::Teal => "Teal",
        Scale::Jade => "Jade",
        Scale::Green => "Green",
        Scale::Grass => "Grass",
        Scale::Brown => "Brown",
        Scale::Bronze => "Bronze",
        Scale::Gold => "Gold",
        Scale::Sky => "Sky",
        Scale::Mint => "Mint",
        Scale::Lime => "Lime",
        Scale::Yellow => "Yellow",
        Scale::Amber => "Amber",
        Scale::Orange => "Orange",
    })
}

/// The host's name for a gray family of `DESIGN.md`.
pub fn gray_name(gray: GrayFamily) -> HostText {
    HostText(match gray {
        GrayFamily::Gray => "Gray",
        GrayFamily::Mauve => "Mauve",
        GrayFamily::Slate => "Slate",
        GrayFamily::Sage => "Sage",
        GrayFamily::Olive => "Olive",
        GrayFamily::Sand => "Sand",
    })
}

/// Why the host refused a widget color, naming the colors involved (FR-035). The previous
/// choice stays.
pub fn refusal(refusal: Refusal) -> HostLine {
    let line = match refusal {
        Refusal::TooClose {
            widget,
            other,
            degrees,
        } => {
            let what = match other {
                Other::Host(_) => "the host color".to_owned(),
                Other::Ok => format!("the color of the {} status", Msg::StatusOk.text().0),
                Other::Warning => {
                    format!("the color of the {} status", Msg::StatusWarning.text().0)
                }
                Other::Error => format!("the color of the {} status", Msg::StatusError.text().0),
            };
            let close = if widget == other.scale() {
                format!("{} is {what}.", scale_name(widget).0)
            } else {
                format!(
                    "{} is {degrees}° of hue from {}, {what}.",
                    scale_name(widget).0,
                    scale_name(other.scale()).0
                )
            };
            format!(
                "{close} A widget color stays at least {HUE_DISTANCE}° from the host color and the status colors, so the appearance was not changed."
            )
        }
        Refusal::NotChromatic { scale } => format!(
            "{} has too little color to be told apart by its hue, so the appearance was not changed.",
            scale_name(scale).0
        ),
    };
    HostLine(line.into())
}

/// A sentence in the host's words put together from parts of the catalog; only this module
/// makes one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostLine(gpui::SharedString);

impl HostLine {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<HostLine> for gpui::SharedString {
    fn from(line: HostLine) -> gpui::SharedString {
        line.0
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
