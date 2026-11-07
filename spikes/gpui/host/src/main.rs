//! The spike's executable. Environment variables, for measurement and tests only:
//! - `SPIKE_DATA_DIR`: where the spike keeps its grant record, layout and settings.
//! - `SPIKE_EXIT_AFTER_MS`: quits after this many milliseconds.
//! - `SPIKE_DRAG_BENCH=1`: drags the first widget by script for 5 s, prints frame times, quits.
//! - `SPIKE_REQUEST_ADD=<entry>`: asks to add a catalog entry at startup, as its toolbar button
//!   does, so the approval dialog opens when the package has no approval.
//!
//! `--dev-approve`: see [`dashboard_gpui_spike::dev_approve`]; it exits once done.

use std::time::{Duration, Instant};

use dashboard_gpui_spike::{dashboard, plugins};
use gpui_kit::*;

fn main() {
    let started = Instant::now();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let data_dir = plugins::data_dir();

    if std::env::args().any(|arg| arg == "--dev-approve") {
        match dashboard_gpui_spike::dev_approve(&data_dir) {
            Ok(()) => {
                eprintln!(
                    "spike: --dev-approve granted the embedded packages in {}",
                    data_dir.display()
                );
                return;
            }
            Err(error) => {
                eprintln!("spike: --dev-approve failed: {error:#}");
                std::process::exit(1);
            }
        }
    }

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            dashboard::bind_keys(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.0), px(820.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Dashboard (GPUI spike)".into()),
                    ..Default::default()
                }),
                app_id: Some("dev.philsantos.dashboard-gpui-spike".into()),
                ..Default::default()
            };
            let opened = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| dashboard::Dashboard::new(data_dir, started, window, cx))
            });
            let (window, dashboard) = match opened {
                Ok(opened) => opened,
                Err(error) => {
                    no_graphics(&format!("{error:#}"));
                    cx.quit();
                    return;
                }
            };

            if let Some(entry) = std::env::var("SPIKE_REQUEST_ADD")
                .ok()
                .and_then(|key| plugins::entry(&key))
            {
                window
                    .update(cx, |_, window, cx| {
                        dashboard.update(cx, |d, cx| d.request_add(entry, window, cx))
                    })
                    .ok();
            }
            if let Some(ms) = std::env::var("SPIKE_EXIT_AFTER_MS")
                .ok()
                .and_then(|ms| ms.parse::<u64>().ok())
            {
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(ms))
                        .await;
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
            if std::env::var_os("SPIKE_DRAG_BENCH").is_some() {
                cx.spawn(async move |cx| {
                    // Let the widgets render first
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    window
                        .update(cx, |_, window, cx| {
                            dashboard.update(cx, |dashboard, cx| dashboard.start_bench(window, cx))
                        })
                        .ok();
                })
                .detach();
            }
        });
}

/// No adapter can draw the window: say so plainly, on the terminal and, on Linux, in a desktop
/// dialog when a dialog tool is installed, since the app cannot draw one itself.
fn no_graphics(detail: &str) {
    let message = format!(
        "Dashboard cannot start: this computer offers no graphics driver it can draw with.\n\n\
         It needs Vulkan, or OpenGL 4.3 / OpenGL ES 3.1 with compute shaders. On Linux, installing \
         the Mesa Vulkan drivers (mesa-vulkan-drivers on Debian and Ubuntu) provides a software \
         renderer that works on any machine.\n\nDetail: {detail}"
    );
    eprintln!("{message}");
    #[cfg(target_os = "linux")]
    {
        let tries: [(&str, Vec<String>); 2] = [
            (
                "zenity",
                vec![
                    "--error".into(),
                    "--title=Dashboard".into(),
                    format!("--text={message}"),
                ],
            ),
            (
                "kdialog",
                vec![
                    "--title".into(),
                    "Dashboard".into(),
                    "--error".into(),
                    message.clone(),
                ],
            ),
        ];
        for (program, args) in tries {
            if std::process::Command::new(program)
                .args(&args)
                .status()
                .is_ok()
            {
                break;
            }
        }
    }
}
