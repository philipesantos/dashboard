//! What the host sets up before its first window (research R4, R9, R12): its own fonts, an HTTP
//! client that refuses every request, and a prompt builder that refuses every prompt, so nothing
//! in GPUI can reach the network or show a native dialog even through a path `clippy.toml` misses.

use gpui::http_client::BlockedHttpClient;
use gpui::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, PromptButton,
    PromptHandle, PromptLevel, PromptResponse, Render, RenderablePromptHandle, Window, div,
};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

/// The host's fonts, from `design/fonts/` (feature 007, research R14).
const FONTS: [&[u8]; 5] = [
    include_bytes!("../../design/fonts/barlow/Barlow-Regular.ttf"),
    include_bytes!("../../design/fonts/barlow/Barlow-Medium.ttf"),
    include_bytes!("../../design/fonts/barlow/Barlow-SemiBold.ttf"),
    include_bytes!("../../design/fonts/plex-mono/IBMPlexMono-Regular.ttf"),
    include_bytes!("../../design/fonts/plex-mono/IBMPlexMono-Medium.ttf"),
];

/// The answer GPUI's prompt receives from the host: no button.
pub const REFUSED_PROMPT: usize = usize::MAX;

/// Stands in for any prompt GPUI is asked for: draws nothing and answers [`REFUSED_PROMPT`] at
/// once. The host asks every decision through its own dialogs (Principle VII, HS-12).
struct RefusedPrompt {
    focus: FocusHandle,
}

impl EventEmitter<PromptResponse> for RefusedPrompt {}

impl Focusable for RefusedPrompt {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for RefusedPrompt {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn refuse_prompt(
    _: PromptLevel,
    _: &str,
    _: Option<&str>,
    _: &[PromptButton],
    handle: PromptHandle,
    window: &mut Window,
    cx: &mut App,
) -> RenderablePromptHandle {
    let view = cx.new(|cx| RefusedPrompt {
        focus: cx.focus_handle(),
    });
    let answer = view.clone();
    // After GPUI has stored the prompt, so its answer also clears it
    cx.defer(move |cx| answer.update(cx, |_, cx| cx.emit(PromptResponse(REFUSED_PROMPT))));
    handle.with_view(view, window, cx)
}

/// Sets the host up on `cx`: its fonts, a blocked HTTP client, the refusing prompt builder.
pub fn install(cx: &mut App) -> Result<(), String> {
    cx.set_http_client(Arc::new(BlockedHttpClient::new()));
    cx.set_prompt_builder(refuse_prompt);
    crate::dialog::init(cx);
    // Quitting answers an open host dialog as a refusal first (HS-10)
    cx.on_app_quit(|cx| {
        crate::dialog::refuse_if_open(cx);
        async {}
    })
    .detach();
    cx.text_system()
        .add_fonts(FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
        .map_err(|error| format!("the host's fonts did not load: {error}"))
}

/// The command line of `dashboard` (contracts/host-binary.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// The development grant record.
    pub grants: PathBuf,
    /// The data directory, as the development tool's `--data`.
    pub data: PathBuf,
}

/// Exit statuses of `dashboard` (contracts/host-binary.md).
pub mod exit {
    pub const USAGE: u8 = 2;
    pub const NO_GPU: u8 = 3;
    pub const NO_RUNTIME: u8 = 4;
}

/// Reads `--grants <path>` and `--data <dir>`; every other argument is a usage error. Missing
/// options take the development tool's defaults.
pub fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut grants = None;
    let mut data = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "--grants" => &mut grants,
            "--data" => &mut data,
            other => return Err(format!("unknown argument: {other}")),
        };
        let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
        *slot = Some(PathBuf::from(value));
    }
    let grants = match grants {
        Some(path) => path,
        None => runtime::grants::GrantStore::default_development_path()
            .ok_or("this system has no data directory: give --grants")?,
    };
    let data = match data {
        Some(path) => path,
        None => runtime::RuntimeOptions::default_development_data_dir()
            .ok_or("this system has no data directory: give --data")?,
    };
    Ok(Options { grants, data })
}
