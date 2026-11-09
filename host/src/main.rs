//! `dashboard`: opens the host window (contracts/host-binary.md).

use gpui::{
    App, AppContext, Bounds, SharedString, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use host::app::{exit, install, parse_options};
use host::dashboard::Dashboard;
use host::plugins::start_runtime;
use host::text::Msg;
use runtime::grants::GrantStore;
use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(problem) => {
            eprintln!("error: {problem}\nusage: dashboard [--grants <path>] [--data <dir>]");
            return ExitCode::from(exit::USAGE);
        }
    };
    let runtime = match start_runtime(&options.data) {
        Ok(runtime) => Arc::new(runtime),
        Err(problem) => {
            eprintln!("error: {problem}");
            return ExitCode::from(exit::NO_RUNTIME);
        }
    };
    let grants = Arc::new(GrantStore::open(&options.grants));
    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(problem) = install(cx) {
            eprintln!("error: {problem}");
            std::process::exit(i32::from(exit::NO_RUNTIME));
        }
        host::vendor::gpui_base::init(cx);
        // The choice saved last time, or the default with a notice when it cannot be used
        let settings = host::appearance::default_path().map_or_else(
            host::appearance::Settings::default,
            host::appearance::Settings::from_file,
        );
        host::appearance::init(cx, settings);
        let window_options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from(Msg::AppName.text())),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.0), px(800.0)),
                cx,
            ))),
            ..Default::default()
        };
        let opened = cx.open_window(window_options, |window, cx| {
            cx.new(|cx| Dashboard::new(Some(runtime), grants, window, cx))
        });
        if let Err(error) = opened {
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
