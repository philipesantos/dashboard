//! The appearance settings (US7, FR-034 to FR-037, research R11): what the person picks, the
//! host file that keeps it, and the host screen that changes it. The screen is drawn inside the
//! content area like a panel of the host, never over the chrome; it decides nothing about trust,
//! so it is not a dialog.
//!
//! The choice is one global for the whole app, so a pick applies at once to every host window and
//! to the dialog of any of them.

use crate::text::{HostText, Msg, gray_name, refusal, scale_name};
use crate::theme::{Theme, body_md, color, label_mono, title_sm};
use crate::ui::{control, text};
use design_tokens::{
    Appearance, GrayFamily, HostColor, Radius, Refusal, Scale, ThemePick, role_color,
};
use gpui::{
    App, Div, FocusHandle, Global, Hsla, InteractiveElement, KeyDownEvent, ParentElement, Role,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Toggled, Window, div, px,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The file's format version (data-model, "Appearance choice").
pub const FILE_VERSION: u64 = 1;

/// The most the host reads of its file; a longer one is not the host's.
const MAX_FILE: u64 = 4096;

/// `<data dir>/dev.philsantos.dashboard/host/appearance.json`, outside every plugin's store
/// (contracts/host-binary.md), or `None` when the system has no data directory.
pub fn default_path() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| {
        dir.join(runtime::grants::APP_DIR)
            .join("host")
            .join("appearance.json")
    })
}

/// One choice on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pick {
    Gray(GrayFamily),
    Host(HostColor),
    Widget(Scale),
    Radius(Radius),
    Theme(ThemePick),
}

impl Pick {
    /// `appearance` with this choice made.
    pub fn apply(self, appearance: Appearance) -> Appearance {
        let mut next = appearance;
        match self {
            Pick::Gray(gray) => next.gray = gray,
            Pick::Host(host) => next.host = host,
            Pick::Widget(widget) => next.widget = widget,
            Pick::Radius(radius) => next.radius = radius,
            Pick::Theme(theme) => next.theme = theme,
        }
        next
    }

    /// Whether `appearance` already has this choice.
    pub fn is_current(self, appearance: &Appearance) -> bool {
        self.apply(*appearance) == *appearance
    }

    /// The host's name for it.
    pub fn name(self) -> HostText {
        match self {
            Pick::Gray(gray) => gray_name(gray),
            Pick::Host(HostColor::Monochrome) => Msg::Monochrome.text(),
            Pick::Host(HostColor::Scale(scale)) | Pick::Widget(scale) => scale_name(scale),
            Pick::Radius(radius) => match radius {
                Radius::None => Msg::RadiusNone.text(),
                Radius::Small => Msg::RadiusSmall.text(),
                Radius::Medium => Msg::RadiusMedium.text(),
                Radius::Large => Msg::RadiusLarge.text(),
                Radius::Full => Msg::RadiusFull.text(),
            },
            Pick::Theme(theme) => match theme {
                ThemePick::Light => Msg::ThemeLight.text(),
                ThemePick::Dark => Msg::ThemeDark.text(),
                ThemePick::System => Msg::ThemeSystem.text(),
            },
        }
    }
}

/// One group of choices, in the screen's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub name: Msg,
    pub picks: Vec<Pick>,
}

/// Every choice feature 007 defines (FR-034): the gray families, monochrome and the chromatic
/// scales for the host, the chromatic scales for widgets, the radii and the themes.
pub fn groups() -> Vec<Group> {
    let chromatic = || Scale::ALL.into_iter().filter(|scale| scale.is_chromatic());
    vec![
        Group {
            name: Msg::GroupGray,
            picks: GrayFamily::ALL.into_iter().map(Pick::Gray).collect(),
        },
        Group {
            name: Msg::GroupHost,
            picks: std::iter::once(HostColor::Monochrome)
                .chain(chromatic().map(HostColor::Scale))
                .map(Pick::Host)
                .collect(),
        },
        Group {
            name: Msg::GroupWidget,
            picks: chromatic().map(Pick::Widget).collect(),
        },
        Group {
            name: Msg::GroupCorners,
            picks: Radius::ALL.into_iter().map(Pick::Radius).collect(),
        },
        Group {
            name: Msg::GroupTheme,
            picks: [ThemePick::Light, ThemePick::Dark, ThemePick::System]
                .into_iter()
                .map(Pick::Theme)
                .collect(),
        },
    ]
}

/// How many choices the screen shows.
pub fn pick_count() -> usize {
    groups().iter().map(|group| group.picks.len()).sum()
}

fn theme_name(theme: ThemePick) -> &'static str {
    match theme {
        ThemePick::Light => "light",
        ThemePick::Dark => "dark",
        ThemePick::System => "system",
    }
}

/// The file's text for `appearance`.
pub fn to_json(appearance: &Appearance) -> String {
    let host = match appearance.host {
        HostColor::Monochrome => "monochrome",
        HostColor::Scale(scale) => scale.name(),
    };
    let value = json!({
        "version": FILE_VERSION,
        "gray": appearance.gray.name(),
        "host": host,
        "widget": appearance.widget.name(),
        "radius": appearance.radius.name(),
        "theme": theme_name(appearance.theme),
    });
    serde_json::to_string_pretty(&value).expect("a JSON object") + "\n"
}

/// The appearance a file holds, if it is one the host can use: the version it writes, every field
/// one of feature 007's names, and a choice `Appearance::validate` accepts.
pub fn from_json(text: &str) -> Option<Appearance> {
    let value: Value = serde_json::from_str(text).ok()?;
    let object = value.as_object()?;
    if object.len() != 6 || object.get("version")?.as_u64()? != FILE_VERSION {
        return None;
    }
    let field = |name: &str| object.get(name).and_then(Value::as_str);
    let scale = |name: &str| Scale::ALL.into_iter().find(|scale| scale.name() == name);
    let (gray, host, widget, radius, theme) = (
        field("gray")?,
        field("host")?,
        field("widget")?,
        field("radius")?,
        field("theme")?,
    );
    let gray = GrayFamily::ALL
        .into_iter()
        .find(|family| family.name() == gray)?;
    let host = match host {
        "monochrome" => HostColor::Monochrome,
        name => HostColor::Scale(scale(name)?),
    };
    let widget = scale(widget)?;
    let radius = Radius::ALL.into_iter().find(|pick| pick.name() == radius)?;
    let theme = [ThemePick::Light, ThemePick::Dark, ThemePick::System]
        .into_iter()
        .find(|pick| theme_name(*pick) == theme)?;
    let appearance = Appearance {
        gray,
        host,
        widget,
        radius,
        theme,
    };
    appearance.validate().ok()?;
    Some(appearance)
}

/// The choice in `path`: none saved yet gives the default quietly; a file the host cannot read or
/// use gives the default and the notice that says so (FR-036).
pub fn load(path: &Path) -> (Appearance, Option<Msg>) {
    use std::io::Read;
    let unusable = (Appearance::DEFAULT, Some(Msg::AppearanceDefaulted));
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Appearance::DEFAULT, None);
        }
        Err(_) => return unusable,
    };
    let mut text = String::new();
    if file.take(MAX_FILE + 1).read_to_string(&mut text).is_err() || text.len() as u64 > MAX_FILE {
        return unusable;
    }
    match from_json(&text) {
        Some(appearance) => (appearance, None),
        None => unusable,
    }
}

/// Writes the choice to `path`, through a file beside it, so a crash never leaves half of one.
pub fn save(path: &Path, appearance: &Appearance) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, to_json(appearance))?;
    std::fs::rename(&partial, path)
}

/// The app's appearance: the choice, the file that keeps it (none in tests that keep nothing),
/// and the notice the host shows about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub appearance: Appearance,
    pub file: Option<PathBuf>,
    pub notice: Option<Msg>,
}

impl Global for Settings {}

impl Default for Settings {
    fn default() -> Settings {
        Settings::fixed(Appearance::DEFAULT)
    }
}

impl Settings {
    /// The choice saved in `path`, kept there from now on.
    pub fn from_file(path: PathBuf) -> Settings {
        let (appearance, notice) = load(&path);
        Settings {
            appearance,
            file: Some(path),
            notice,
        }
    }

    /// `appearance`, kept nowhere.
    pub fn fixed(appearance: Appearance) -> Settings {
        Settings {
            appearance,
            file: None,
            notice: None,
        }
    }
}

/// Sets the app's appearance, before its first window.
pub fn init(cx: &mut App, settings: Settings) {
    cx.set_global(settings);
}

/// The appearance every host surface draws with now.
pub fn current(cx: &App) -> Appearance {
    cx.try_global::<Settings>()
        .map_or(Appearance::DEFAULT, |settings| settings.appearance)
}

/// The host's notice about the appearance, if it has one.
pub fn notice(cx: &App) -> Option<Msg> {
    cx.try_global::<Settings>()
        .and_then(|settings| settings.notice)
}

/// Makes `pick`: refused with the host's reason and nothing changed when `Appearance::validate`
/// refuses the result (FR-035); otherwise applied to every host surface and saved (FR-036).
pub fn choose(cx: &mut App, pick: Pick) -> Result<(), Refusal> {
    let next = pick.apply(current(cx));
    next.validate()?;
    let settings = cx.default_global::<Settings>();
    settings.appearance = next;
    settings.notice = None;
    if let Some(file) = settings.file.clone()
        && save(&file, &next).is_err()
    {
        settings.notice = Some(Msg::AppearanceNotSaved);
    }
    Ok(())
}

/// The screen's focus handles, kept by each window for as long as it is open, so focus given
/// before the screen is drawn is kept.
pub struct Focus {
    pub close: FocusHandle,
    pub picks: Vec<FocusHandle>,
}

impl Focus {
    pub fn new(cx: &mut App) -> Focus {
        Focus {
            close: cx.focus_handle().tab_stop(true),
            picks: (0..pick_count())
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
        }
    }

    /// The handle of the first current choice: where focus goes when the screen opens.
    pub fn first_current(&self, appearance: &Appearance) -> &FocusHandle {
        let index = groups()
            .iter()
            .flat_map(|group| group.picks.iter())
            .position(|pick| pick.is_current(appearance))
            .unwrap_or(0);
        &self.picks[index]
    }
}

/// A small sample of the color a choice makes, in the screen's theme; none for the radius and
/// the theme. Monochrome has no color of its own: a ring in the label's color, `ink`.
fn swatch(pick: Pick, appearance: &Appearance, theme: &Theme, ink: Hsla) -> Option<Div> {
    let dot = div().flex_none().size(px(10.0)).rounded(px(5.0));
    if pick == Pick::Host(HostColor::Monochrome) {
        return Some(dot.border(px(1.5)).border_color(ink));
    }
    let kind = if theme.dark {
        design_tokens::Theme::Dark
    } else {
        design_tokens::Theme::Light
    };
    let sample = |role: &str, appearance: Appearance| role_color(role, &appearance, kind);
    let rgb = match pick {
        Pick::Gray(gray) => sample(
            "text-secondary",
            Appearance {
                gray,
                ..*appearance
            },
        ),
        Pick::Host(HostColor::Monochrome) | Pick::Radius(_) | Pick::Theme(_) => None,
        Pick::Host(HostColor::Scale(scale)) | Pick::Widget(scale) => sample(
            "accent-text",
            Appearance {
                widget: scale,
                ..*appearance
            },
        ),
    }?;
    Some(dot.bg(color(rgb)))
}

/// A line of the host's words that assistive technology reads as one node.
fn line(id: &'static str, theme: &Theme, words: impl Into<SharedString>) -> Stateful<Div> {
    let words: SharedString = words.into();
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(words.clone())
        .child(text(body_md(), theme.text_secondary, words))
}

/// The host's notice about the appearance, at the top of the content area.
pub fn notice_line(theme: &Theme, notice: Msg) -> Stateful<Div> {
    let words: SharedString = notice.text().into();
    div()
        .id("appearance-notice")
        .debug_selector(|| "appearance-notice".to_owned())
        .role(Role::Label)
        .aria_label(words.clone())
        .mb(px(12.0))
        .pl(px(8.0))
        .border_l(px(2.0))
        .border_color(theme.ink)
        .child(text(body_md(), theme.text, words))
}

/// What pressing a choice, or closing the screen, does.
pub type OnPick = Rc<dyn Fn(Pick, &mut Window, &mut App)>;
pub type OnClose = Rc<dyn Fn(&mut Window, &mut App)>;

/// The appearance screen: one group per choice with the current one marked, the host's refusal
/// of the last choice if it refused one, and "Close". Escape closes it too.
pub fn screen(
    theme: &Theme,
    window: &Window,
    appearance: &Appearance,
    focus: &Focus,
    refused: Option<Refusal>,
    on_pick: OnPick,
    on_close: OnClose,
) -> Stateful<Div> {
    let on_escape = on_close.clone();
    let mut index = 0;
    let groups = groups().into_iter().enumerate().map(|(number, group)| {
        let name: SharedString = group.name.text().into();
        let picks = group.picks.into_iter().map(|pick| {
            let current = pick.is_current(appearance);
            let on_pick = on_pick.clone();
            let (face, ink) = if current {
                (theme.ink, theme.on_ink)
            } else {
                (theme.neutral_soft, theme.text)
            };
            let element = control(
                ("appearance-pick", index),
                &focus.picks[index],
                theme,
                window,
                pick.name(),
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(theme.radius)
                    .bg(face)
                    .border(px(1.0))
                    .border_color(if current {
                        theme.ink
                    } else {
                        theme.border_control
                    })
                    .children(swatch(pick, appearance, theme, ink))
                    .child(text(label_mono(), ink, pick.name())),
                move |window, cx| on_pick(pick, window, cx),
            )
            .role(Role::RadioButton)
            .aria_toggled(if current {
                Toggled::True
            } else {
                Toggled::False
            });
            index += 1;
            element
        });
        div()
            .id(("appearance-group", number))
            .role(Role::RadioGroup)
            .aria_label(name.clone())
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(text(label_mono(), theme.text_secondary, name))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.0))
                    .children(picks.collect::<Vec<_>>()),
            )
    });
    let groups: Vec<_> = groups.collect();
    div()
        .id("appearance")
        .debug_selector(|| "appearance".to_owned())
        .role(Role::Group)
        .aria_label(SharedString::from(Msg::Appearance.text()))
        .flex()
        .flex_col()
        .gap(px(16.0))
        .p(px(16.0))
        .max_w(px(720.0))
        .bg(theme.panel)
        .border(px(1.0))
        .border_color(theme.divider)
        .rounded(theme.radius)
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                on_escape(window, cx);
                cx.stop_propagation();
            }
        })
        .child(
            div()
                .flex()
                .items_center()
                .child(text(title_sm(), theme.text, Msg::Appearance.text()))
                .child(div().flex_1())
                .child(control(
                    "appearance-close",
                    &focus.close,
                    theme,
                    window,
                    Msg::Close.text(),
                    div()
                        .px(px(8.0))
                        .py(px(2.0))
                        .rounded(theme.radius)
                        .bg(theme.neutral_soft)
                        .border(px(1.0))
                        .border_color(theme.border_control)
                        .child(text(label_mono(), theme.text, Msg::Close.text())),
                    move |window, cx| on_close(window, cx),
                )),
        )
        .child(line("appearance-hint", theme, Msg::AppearanceHint.text()))
        .children(refused.map(|refused| {
            let words: SharedString = refusal(refused).into();
            div()
                .id("appearance-refusal")
                .debug_selector(|| "appearance-refusal".to_owned())
                .role(Role::Label)
                .aria_label(words.clone())
                .pl(px(8.0))
                .border_l(px(2.0))
                .border_color(theme.error_icon)
                .child(text(body_md(), theme.text, words))
        }))
        .children(groups)
}
