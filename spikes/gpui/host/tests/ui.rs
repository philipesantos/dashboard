//! The spike's UI, driven headless through GPUI's test platform: keyboard-only use of the grid
//! and the approval dialog, the accessibility roles and names of the host's chrome, and widget
//! content that cannot leave its frame.

use std::path::Path;
use std::time::Instant;

use dashboard_gpui_spike::{dashboard::Dashboard, plugins};
use gpui_kit::base::Root;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AppContext, Bounds, ElementId, Entity, Point, Role, SharedString, TestAppContext, WindowBounds,
    WindowHandle, WindowOptions, px, size,
};

fn open(cx: &mut TestAppContext, data_dir: &Path) -> (WindowHandle<Root>, Entity<Dashboard>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        dashboard_gpui_spike::dashboard::bind_keys(cx);
    });
    let dir = data_dir.to_owned();
    let (window, dashboard) = cx.update(|cx| {
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(1280.0), px(820.0)),
        };
        let (window, dashboard) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            move |window, cx| cx.new(|cx| Dashboard::new(dir, Instant::now(), window, cx)),
        )
        .expect("open test window");
        (window.downcast::<Root>().expect("Base Root"), dashboard)
    });
    cx.run_until_parked();
    (window, dashboard)
}

/// A data directory with the two packages approved and four widgets laid out.
fn approved() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    dashboard_gpui_spike::dev_approve(dir.path()).unwrap();
    dir
}

/// A data directory with a visual secret chosen and nothing approved.
fn fresh() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{"mode":"Light","secret":"Teal"}"#,
    )
    .unwrap();
    dir
}

fn frame(id: usize) -> ElementId {
    ElementId::from(("frame", id))
}

fn named(name: &str, index: usize) -> ElementId {
    ElementId::from((SharedString::from(name.to_owned()), index))
}

#[gpui_kit::test]
fn tab_reaches_every_frame_and_widget_button(cx: &mut TestAppContext) {
    let dir = approved();
    let (window, _) = open(cx, dir.path());
    let _ = cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        let mut targets: Vec<(String, ElementId)> =
            (0..4).map(|i| (format!("frame {i}"), frame(i))).collect();
        targets.push(("sampler's button".into(), named("w0-button", 5)));
        targets.push(("spoof's button".into(), named("w3-button", 2)));
        targets.push(("Settings".into(), "settings".into()));
        targets.push(("Add Hello".into(), "add-hello".into()));
        let mut reached = vec![false; targets.len()];
        for _ in 0..40 {
            window.press("tab", cx);
            for (i, (_, id)) in targets.iter().enumerate() {
                if window.try_find(id.clone()).and_then(|e| e.focused()) == Some(true) {
                    reached[i] = true;
                }
            }
        }
        let missed: Vec<&str> = targets
            .iter()
            .zip(&reached)
            .filter(|(_, r)| !**r)
            .map(|((n, _), _)| n.as_str())
            .collect();
        assert!(missed.is_empty(), "Tab never reached: {missed:?}");
    });
}

#[gpui_kit::test]
fn arrow_keys_move_and_resize_the_focused_widget(cx: &mut TestAppContext) {
    let dir = approved();
    let (window, dashboard) = open(cx, dir.path());
    let _ = cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        // Tab to the Hello widget's frame (widget 1, at column 6, row 1)
        for _ in 0..40 {
            if window.find(frame(1)).focused() == Some(true) {
                break;
            }
            window.press("tab", cx);
        }
        assert_eq!(window.find(frame(1)).focused(), Some(true));
        assert_eq!(
            window.find(frame(1)).label(),
            Some("Hello, plugin local/hello-widget. Column 6, row 1, 3 by 2 cells.")
        );
        assert_eq!(window.find(frame(1)).role(), Some(Role::Group));

        window.press("down", cx);
        window.press("shift-left", cx); // narrower: 3 → 2 columns
        assert_eq!(
            window.find(frame(1)).label(),
            Some("Hello, plugin local/hello-widget. Column 6, row 2, 2 by 2 cells.")
        );
        // Left would land on the sampler: refused, and said so
        window.press("left", cx);
        assert_eq!(
            window.find(frame(1)).label(),
            Some("Hello, plugin local/hello-widget. Column 6, row 2, 2 by 2 cells.")
        );
        let status = window.find("status");
        assert_eq!(status.role(), Some(Role::Status));
        assert_eq!(status.label(), Some("That place is taken or off the grid."));

        window.press("delete", cx);
        assert!(window.try_find(frame(1)).is_none());
    });
    let saved = std::fs::read_to_string(dir.path().join("layout.json")).unwrap();
    assert!(!saved.contains("\"hello\""), "the removal was saved");
    cx.update(|cx| assert_eq!(dashboard.read(cx).layout().widgets.len(), 3));
}

#[gpui_kit::test]
fn the_approval_dialog_keeps_focus_and_escape_cancels(cx: &mut TestAppContext) {
    let dir = fresh();
    let (window, dashboard) = open(cx, dir.path());
    let _ = cx.update_window(window.into(), |_, window, cx| {
        dashboard.update(cx, |d, cx| {
            d.request_add(plugins::entry("sampler").unwrap(), window, cx)
        });
        window.render_frame(cx);
        let dialog = window.find("host-dialog");
        assert_eq!(dialog.role(), Some(Role::Dialog));
        assert_eq!(dialog.label(), Some("Add Release status?"));
        assert_eq!(
            window.find("visual-secret").label(),
            Some("Your color, Teal: this dialog is from Dashboard")
        );
        assert_eq!(
            window.find_by_label("log (required)").role(),
            Some(Role::ListItem)
        );

        // Tab and Shift-Tab stay on the dialog's two buttons
        for key in ["tab", "tab", "tab", "shift-tab", "shift-tab", "tab"] {
            window.press(key, cx);
            let inside = window.find("approval-cancel").focused() == Some(true)
                || window.find("approval-approve").focused() == Some(true);
            assert!(inside, "focus left the dialog after {key}");
            assert_ne!(window.find("add-hello").focused(), Some(true));
        }
        window.press("escape", cx);
        assert!(window.try_find("host-dialog").is_none());
    });
    cx.update(|cx| assert!(dashboard.read(cx).layout().widgets.is_empty()));

    // Again, and approve with the keyboard alone
    let _ = cx.update_window(window.into(), |_, window, cx| {
        dashboard.update(cx, |d, cx| {
            d.request_add(plugins::entry("sampler").unwrap(), window, cx)
        });
        window.render_frame(cx);
        for _ in 0..4 {
            if window.find("approval-approve").focused() == Some(true) {
                break;
            }
            window.press("tab", cx);
        }
        window.press("enter", cx);
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let d = dashboard.read(cx);
        assert!(!d.dialog_open());
        assert_eq!(d.layout().widgets.len(), 1);
        assert!(d.widget_ready(0), "error: {:?}", d.widget_error(0));
    });
}

#[gpui_kit::test]
fn widget_content_is_drawn_by_the_host_and_clipped_by_the_frame(cx: &mut TestAppContext) {
    let dir = approved();
    let (window, _) = open(cx, dir.path());
    let _ = cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        // The sampler's blocks, as the host's components
        assert_eq!(window.find(named("w0-text", 0)).role(), Some(Role::Heading));
        assert_eq!(window.find(named("w0-list", 2)).role(), Some(Role::List));
        assert_eq!(window.find(named("w0-table", 3)).role(), Some(Role::Table));
        assert_eq!(
            window.find(named("w0-button", 5)).role(),
            Some(Role::Button)
        );
        assert_eq!(window.find(named("w0-button", 5)).label(), Some("Press"));

        // Overflow widget: its first item shows, its last is clipped away, and nothing it
        // draws reaches outside its frame's body
        let body = window.find(named("frame-body", 2)).bounds();
        assert!(
            window.find(named("w2-list2-item", 0)).visible()
                || window.find(named("w2-text", 0)).visible()
        );
        let last = window.find(named("w2-list2-item", 79));
        assert!(
            last.bounds().bottom() > body.bottom(),
            "the list is taller than the frame"
        );
        assert!(!last.visible(), "the part outside the frame is not drawn");
        for item in (0..80)
            .map(|n| window.find(named("w2-list2-item", n)))
            .filter(|e| e.visible())
        {
            assert!(item.bounds().top() < body.bottom() && item.bounds().bottom() > body.top());
        }
    });
}

#[gpui_kit::test]
fn the_kit_refuses_what_it_does_not_offer(cx: &mut TestAppContext) {
    let dir = approved();
    let (window, dashboard) = open(cx, dir.path());
    let _ = cx.update_window(window.into(), |_, window, cx| {
        dashboard.update(cx, |d, cx| {
            d.request_add(plugins::entry("password").unwrap(), window, cx);
            d.request_add(plugins::entry("host-color").unwrap(), window, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let d = dashboard.read(cx);
        let password = d.widget_error(4).expect("refused");
        assert!(
            password.contains("unknown variant `password-field`"),
            "{password}"
        );
        let color = d.widget_error(5).expect("refused");
        assert!(color.contains("unknown variant `host-secret`"), "{color}");
    });
    let _ = cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(named("widget-error", 4)).role(),
            Some(Role::Alert)
        );
    });
}
