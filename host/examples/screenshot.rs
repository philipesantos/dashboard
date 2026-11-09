//! Draws the dashboard offscreen with GPUI's headless renderer and saves it as a PNG, for
//! reviewing the look without a display (specs/008-host-window-dialogs, T053).
//!
//! `cargo run -p host --example screenshot -- --grants <path> --data <dir> --out <file.png>
//! [--dark] [--size <w>x<h>] [--dialog] [--appearance] [--refuse <widget color>]`
//!
//! `--dialog` asks "Remove from dashboard" for the first frame before the screenshot;
//! `--appearance` opens the appearance screen, and `--refuse` then picks a widget color, which
//! shows the host's refusal when the rule refuses it. `--time` prints how long after the program
//! started every approved plugin's frame had its status, and each package's load (compile) time
//! (SC-001, research R14), measured offscreen with GPUI's headless renderer.

use design_tokens::{Appearance, ThemePick};
use gpui::{AppContext as _, HeadlessAppContext, px, size};
use host::app::install;
use host::dashboard::Dashboard;
use host::plugins::start_runtime;
use runtime::grants::GrantStore;
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<(), String> {
    let mut grants = None;
    let mut data = None;
    let mut out = PathBuf::from("dashboard.png");
    let mut dark = false;
    let mut with_dialog = false;
    let mut with_appearance = false;
    let mut refuse = None;
    let mut time = false;
    let started = std::time::Instant::now();
    let (mut width, mut height) = (1280.0, 800.0);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--grants" => grants = args.next().map(PathBuf::from),
            "--data" => data = args.next().map(PathBuf::from),
            "--out" => out = PathBuf::from(args.next().ok_or("--out needs a file")?),
            "--dark" => dark = true,
            "--dialog" => with_dialog = true,
            "--appearance" => with_appearance = true,
            "--time" => time = true,
            "--refuse" => {
                let name = args.next().ok_or("--refuse needs a color")?;
                refuse = Some(
                    design_tokens::Scale::ALL
                        .into_iter()
                        .find(|scale| scale.name() == name)
                        .ok_or("--refuse needs a color of DESIGN.md")?,
                );
            }
            "--size" => {
                let value = args.next().ok_or("--size needs <w>x<h>")?;
                let (w, h) = value.split_once('x').ok_or("--size needs <w>x<h>")?;
                width = w.parse().map_err(|_| "a width in pixels")?;
                height = h.parse().map_err(|_| "a height in pixels")?;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let grants = Arc::new(GrantStore::open(grants.ok_or("--grants is required")?));
    let runtime = Arc::new(start_runtime(&data.ok_or("--data is required")?)?);
    let appearance = Appearance {
        theme: if dark {
            ThemePick::Dark
        } else {
            ThemePick::Light
        },
        ..Appearance::DEFAULT
    };

    let platform = gpui_platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        install(cx)?;
        host::appearance::init(cx, host::appearance::Settings::fixed(appearance));
        host::vendor::gpui_base::init(cx);
        Ok::<(), String>(())
    })?;
    let window = cx
        .open_window(size(px(width), px(height)), |window, cx| {
            cx.new(|cx| Dashboard::new(Some(runtime), grants, window, cx))
        })
        .map_err(|error| format!("the window did not open: {error:#}"))?;
    cx.run_until_parked();
    if time {
        let widgets = cx
            .update(|cx| window.update(cx, |view, _, _| view.widgets().to_vec()))
            .map_err(|error| format!("the window is gone: {error:#}"))?;
        let pending = widgets
            .iter()
            .filter(|widget| widget.status == host::plugins::FrameStatus::Loading)
            .count();
        println!(
            "every frame drawn with its status: {:?} after start ({} frames, {pending} still loading)",
            started.elapsed(),
            widgets.len()
        );
        for widget in &widgets {
            println!(
                "  {}: {:?}, loaded in {:?}",
                widget.id.as_str(),
                widget.status,
                widget.load_time
            );
        }
    }
    if with_appearance {
        cx.update(|cx| {
            window.update(cx, |view, window, cx| {
                view.toggle_appearance(window, cx);
                if let Some(scale) = refuse {
                    view.pick(host::appearance::Pick::Widget(scale), cx);
                }
            })
        })
        .map_err(|error| format!("the window is gone: {error:#}"))?;
        cx.run_until_parked();
    }
    if with_dialog {
        cx.update(|cx| window.update(cx, |view, window, cx| view.ask_remove(0, window, cx)))
            .map_err(|error| format!("the window is gone: {error:#}"))?;
        cx.run_until_parked();
    }
    let image = cx
        .capture_screenshot(window.into())
        .map_err(|error| format!("no screenshot: {error:#}"))?;
    image
        .save(&out)
        .map_err(|error| format!("{}: {error}", out.display()))?;
    println!("{}", out.display());
    Ok(())
}
