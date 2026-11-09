//! `dashboard`: opens the host window (contracts/host-binary.md).

use gpui::{App, AppContext, Context, IntoElement, Render, Window, WindowOptions, div};
use host::app::{exit, install, parse_options};
use std::process::ExitCode;

struct Empty;

impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn main() -> ExitCode {
    let _options = match parse_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(problem) => {
            eprintln!("error: {problem}\nusage: dashboard [--grants <path>] [--data <dir>]");
            return ExitCode::from(exit::USAGE);
        }
    };
    gpui_platform::application().run(|cx: &mut App| {
        if let Err(problem) = install(cx) {
            eprintln!("error: {problem}");
            std::process::exit(i32::from(exit::NO_RUNTIME));
        }
        if let Err(error) = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty)) {
            eprintln!(
                "error: the window could not open: {error:#}\nThe dashboard draws with the GPU: on Linux it needs a Vulkan or OpenGL 4.3 driver (Mesa's, for example)."
            );
            std::process::exit(i32::from(exit::NO_GPU));
        }
        // Closing the last window quits
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
    });
    ExitCode::SUCCESS
}
