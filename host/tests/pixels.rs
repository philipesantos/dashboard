//! What the window draws, checked in pixels: GPUI's headless renderer draws the dashboard on the
//! GPU without a display (research R13). The scrim covers every pixel of the window, chrome
//! included (FR-015, HS-02), and a focused control shows the host's ring (HS-24). Systems without
//! a headless renderer (Windows) skip with a message; the `window` target checks them in a real
//! window instead.

use design_tokens::{Appearance, Theme as Mode};
use gpui::{AppContext as _, HeadlessAppContext, Rgba, px, size};
use host::dashboard::Dashboard;
use host::theme::Theme;
use image::RgbaImage;
use runtime::grants::GrantStore;
use std::sync::Arc;

mod common;

const WIDTH: f32 = 1024.0;
const HEIGHT: f32 = 640.0;

struct Drawn {
    image: RgbaImage,
    scale: f32,
}

/// Draws the dashboard of `world`, then runs `act` on it, and returns the last frame.
fn draw(
    world: &common::World,
    act: impl FnOnce(&mut Dashboard, &mut gpui::Window, &mut gpui::Context<Dashboard>),
) -> Option<Drawn> {
    let renderer = gpui_platform::current_headless_renderer().ok().flatten();
    if renderer.is_none() {
        eprintln!("pixels: no headless renderer on this system; skipped");
        return None;
    }
    let platform = gpui_platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        host::app::install(cx).expect("installs");
        host::vendor::gpui_base::init(cx);
    });
    let runtime = Arc::new(host::plugins::start_runtime(world.dir.path()).expect("a runtime"));
    let grants: Arc<GrantStore> = Arc::clone(&world.grants);
    let window = cx
        .open_window(size(px(WIDTH), px(HEIGHT)), |window, cx| {
            cx.new(|cx| Dashboard::new(Some(runtime), grants, window, cx))
        })
        .expect("a window");
    cx.run_until_parked();
    cx.update(|cx| window.update(cx, |view, window, cx| act(view, window, cx)))
        .expect("the window");
    cx.run_until_parked();
    let scale = cx
        .update_window(window.into(), |_, window, _| window.scale_factor())
        .expect("the window");
    let image = cx.capture_screenshot(window.into()).expect("a screenshot");
    Some(Drawn { image, scale })
}

fn pixel(drawn: &Drawn, x: f32, y: f32) -> [u8; 4] {
    drawn
        .image
        .get_pixel((x * drawn.scale) as u32, (y * drawn.scale) as u32)
        .0
}

fn rgb(color: gpui::Hsla) -> [f32; 3] {
    let rgba = Rgba::from(color);
    [rgba.r * 255.0, rgba.g * 255.0, rgba.b * 255.0]
}

fn close(actual: [u8; 4], expected: [f32; 3], what: &str) {
    for channel in 0..3 {
        assert!(
            (f32::from(actual[channel]) - expected[channel]).abs() <= 6.0,
            "{what}: drawn {actual:?}, expected about {expected:?}"
        );
    }
}

/// FR-015, HS-02: with a dialog open, the chrome's pixels are the chrome under the scrim, and so
/// are the content's, at every corner and edge of the window.
fn the_scrim_covers_every_pixel() {
    let world = common::World::new();
    world.approve("hello-widget");
    let theme = Theme::new(&Appearance::DEFAULT, Mode::Light);
    let Some(plain) = draw(&world, |_, _, _| {}) else {
        return;
    };
    let Some(dimmed) = draw(&world, |view, window, cx| view.ask_remove(0, window, cx)) else {
        return;
    };
    let scrim = rgb(theme.scrim);
    let alpha = theme.scrim.a;
    let under = |color: [u8; 4]| -> [f32; 3] {
        let mut out = [0.0; 3];
        for channel in 0..3 {
            out[channel] = f32::from(color[channel]) * (1.0 - alpha) + scrim[channel] * alpha;
        }
        out
    };
    // Corners and edge points: the chrome's top corners, mid-content, the bottom corners
    for (x, y, what) in [
        (2.0, 2.0, "the chrome's top-left corner"),
        (WIDTH - 3.0, 2.0, "the chrome's top-right corner"),
        (WIDTH / 2.0, 20.0, "the chrome's middle"),
        (2.0, HEIGHT - 3.0, "the bottom-left corner"),
        (WIDTH - 3.0, HEIGHT - 3.0, "the bottom-right corner"),
    ] {
        close(pixel(&dimmed, x, y), under(pixel(&plain, x, y)), what);
    }
    println!("pixels: the scrim covers the window, chrome included");
}

/// HS-24: focusing a control draws a ring around it, in the host's focus color, outside it.
fn a_focused_control_shows_the_ring() {
    let world = common::World::new();
    let theme = Theme::new(&Appearance::DEFAULT, Mode::Light);
    let Some(plain) = draw(&world, |_, _, _| {}) else {
        return;
    };
    let Some(focused) = draw(&world, |view, window, cx| {
        let reload = view.reload_focus().clone();
        window.focus(&reload, cx);
    }) else {
        return;
    };
    // The chrome's right end holds "Reload"; the ring changes pixels there to the focus color
    let focus = rgb(theme.focus);
    let mut ring = 0;
    let (x0, x1) = ((WIDTH - 140.0) as u32, (WIDTH - 2.0) as u32);
    for x in x0..x1 {
        for y in 2..38 {
            let (x, y) = (x as f32, y as f32);
            let (before, after) = (pixel(&plain, x, y), pixel(&focused, x, y));
            if before != after && (0..3).all(|c| (f32::from(after[c]) - focus[c]).abs() <= 24.0) {
                ring += 1;
            }
        }
    }
    assert!(
        ring > 100,
        "only {ring} pixels of the focus ring around Reload"
    );
    println!("pixels: a focused control shows the ring ({ring} pixels)");
}

fn main() {
    the_scrim_covers_every_pixel();
    a_focused_control_shows_the_ring();
}
