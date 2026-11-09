//! The host's logic, headless: GPUI's test platform where a window is needed, plain functions
//! elsewhere (research R13, layer 1).

mod common;

mod label {
    use host::label::{Label, MAX_LABEL_GRAPHEMES, MONOGRAM_FALLBACK, label, monogram};
    use host::text::Msg;

    fn text(name: &str) -> String {
        label(name).as_str().to_owned()
    }

    /// HS-06: every Cc, Cf, Zl and Zp character is removed.
    #[test]
    fn control_format_and_separator_characters_are_removed() {
        // Cc (NUL, ESC), Cf (RLO, ZWSP, BOM), Zl, Zp
        assert_eq!(
            text("Ta\u{0}s\u{1b}k\u{202e}s\u{200b}\u{feff}\u{2028}!\u{2029}"),
            "Tasks!"
        );
        // A right-to-left override cannot reorder what the frame shows
        assert_eq!(text("\u{202e}ksaT"), "ksaT");
    }

    /// HS-06: more than 40 grapheme clusters are cut at 40 and followed by an ellipsis; clusters
    /// are never split.
    #[test]
    fn long_names_are_capped_at_forty_grapheme_clusters() {
        let long = "e\u{301}".repeat(MAX_LABEL_GRAPHEMES + 5);
        let shown = text(&long);
        assert!(shown.ends_with('…'));
        assert_eq!(
            shown.trim_end_matches('…'),
            "e\u{301}".repeat(MAX_LABEL_GRAPHEMES)
        );
        let exact = "a".repeat(MAX_LABEL_GRAPHEMES);
        assert_eq!(text(&exact), exact, "forty clusters keep no ellipsis");
    }

    /// HS-06: nothing left after cleaning shows the host's "Unnamed widget".
    #[test]
    fn an_empty_result_is_the_hosts_unnamed_widget() {
        for name in ["", "   ", "\u{202e}\u{200b}\u{feff}", "\u{2028}\u{2029}"] {
            assert_eq!(
                label(name),
                Label::Unnamed(Msg::UnnamedWidget.text()),
                "{name:?}"
            );
        }
    }

    /// HS-06: a name imitating the host is drawn like any other, as the plugin's text.
    #[test]
    fn host_imitating_names_stay_plugin_text() {
        for name in ["Dashboard", "System", "Remove a widget", "Unnamed widget"] {
            assert!(matches!(label(name), Label::Plugin(_)), "{name}");
            assert_eq!(text(name), name);
        }
    }

    /// HS-07: the first letter or digit, upper-cased where the script has case, or the host's
    /// fixed glyph.
    #[test]
    fn the_monogram_is_the_first_letter_or_digit() {
        assert_eq!(monogram("tasks"), "T");
        assert_eq!(monogram("🍅 pomodoro"), "P");
        assert_eq!(monogram("★ 2fa codes"), "2");
        assert_eq!(monogram("ωmega"), "Ω");
        assert_eq!(monogram("日本語"), "日");
        assert_eq!(monogram("\u{202e}zeta"), "Z");
        assert_eq!(monogram("★ ☆ !"), MONOGRAM_FALLBACK);
        assert_eq!(monogram(""), MONOGRAM_FALLBACK);
        // A combining mark alone is not a letter
        assert_eq!(monogram("\u{301}x"), "X");
    }
}

mod theme {
    use design_tokens::{Appearance, Theme as Mode, ThemePick};
    use gpui::{Rgba, WindowAppearance};
    use host::theme::{Theme, resolve_theme};

    fn hex(color: gpui::Hsla) -> String {
        let rgba = Rgba::from(color);
        format!(
            "#{:02x}{:02x}{:02x}",
            (rgba.r * 255.0).round() as u8,
            (rgba.g * 255.0).round() as u8,
            (rgba.b * 255.0).round() as u8
        )
    }

    /// FR-006: the default look is DESIGN.md's front matter, light and dark.
    #[test]
    fn the_default_theme_is_design_mds() {
        let light = Theme::new(&Appearance::DEFAULT, Mode::Light);
        assert_eq!(hex(light.canvas), "#f1f0ef");
        assert_eq!(hex(light.surface), "#fdfdfc");
        assert_eq!(hex(light.chrome), "#e9e8e6");
        assert_eq!(hex(light.ink), "#21201c");
        assert_eq!(hex(light.focus), "#21201c");
        assert_eq!(f32::from(light.radius), 2.0);
        let dark = Theme::new(&Appearance::DEFAULT, Mode::Dark);
        assert_eq!(hex(dark.canvas), "#111110");
        assert_eq!(hex(dark.ink), "#eeeeec");
        assert!(dark.scrim.a > 0.0 && dark.scrim.a < 1.0);
    }

    /// FR-037: the system pick follows the window's appearance.
    #[test]
    fn the_system_pick_follows_the_window() {
        assert_eq!(
            resolve_theme(ThemePick::System, WindowAppearance::VibrantDark),
            Mode::Dark
        );
        assert_eq!(
            resolve_theme(ThemePick::System, WindowAppearance::Light),
            Mode::Light
        );
        assert_eq!(
            resolve_theme(ThemePick::Dark, WindowAppearance::Light),
            Mode::Dark
        );
    }
}

mod media {
    use design_tokens::Icon;
    use host::media::status_icon_svg;

    /// The status icons are built from DESIGN.md's path data and the given color only: no
    /// address, no reference outside the document.
    #[test]
    fn status_icons_are_self_contained_svg() {
        let svg = status_icon_svg(Icon::Check, gpui::black());
        assert!(svg.contains(Icon::Check.path()));
        assert!(svg.contains(r##"fill="#000000""##));
        assert!(!svg.contains("href"), "no reference to anything outside");
    }
}

mod app {
    use gpui::TestAppContext;
    use host::app::{install, parse_options};
    use std::path::PathBuf;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    /// contracts/host-binary.md: `--grants` and `--data`, nothing else; no `--connectors`.
    #[test]
    fn options_are_grants_and_data_only() {
        let options = parse_options(args(&["--grants", "/g.json", "--data", "/d"])).expect("valid");
        assert_eq!(options.grants, PathBuf::from("/g.json"));
        assert_eq!(options.data, PathBuf::from("/d"));
        assert!(parse_options(args(&["--connectors", "/c"])).is_err());
        assert!(parse_options(args(&["--grants"])).is_err());
    }

    /// FR-030: the app's HTTP client refuses every request (research R12).
    #[gpui::test]
    async fn the_http_client_refuses(cx: &mut TestAppContext) {
        cx.update(|cx| install(cx).expect("installs"));
        let client = cx.update(|cx| cx.http_client());
        let answer = client
            .get("https://example.com/", Default::default(), false)
            .await;
        assert!(answer.is_err(), "the request was refused");
    }
}

mod focus_trap {
    use gpui::{
        Context, Element as _, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render,
        Role, StatefulInteractiveElement, TestAppContext, Window, accesskit, div,
    };
    use host::vendor::gpui_base::tab::{CONTEXT, Tab, TabPrev, advance_focus};
    use host::vendor::gpui_base::{self, FocusTrapElement};

    /// Ported from gpui-base: the trap passes its inner element's role, label and description
    /// to the node GPUI writes.
    #[gpui::test]
    fn the_trap_keeps_its_inner_semantics(cx: &mut TestAppContext) {
        cx.update(|cx| {
            for (role, name) in [
                (Role::Dialog, "Remove a widget"),
                (Role::AlertDialog, "Quit?"),
            ] {
                let trap = div()
                    .id("inner")
                    .role(role)
                    .aria_label(name)
                    .aria_description("The question")
                    .focus_trap("outer-trap", &cx.focus_handle());
                assert_eq!(gpui::Element::id(&trap), Some("outer-trap".into()));
                assert_eq!(trap.a11y_role(), Some(role));
                let mut node = accesskit::Node::new(role);
                trap.write_a11y_info(&mut node);
                assert_eq!(node.label(), Some(name));
                assert_eq!(node.description(), Some("The question"));
            }
        });
    }

    struct Trapped {
        outside: FocusHandle,
        trap: FocusHandle,
        first: FocusHandle,
        second: FocusHandle,
    }

    impl Render for Trapped {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .key_context(CONTEXT)
                .on_action(cx.listener(|_, _: &Tab, window, cx| {
                    advance_focus(window, cx, Window::focus_next)
                }))
                .on_action(cx.listener(|_, _: &TabPrev, window, cx| {
                    advance_focus(window, cx, Window::focus_prev)
                }))
                .child(
                    div()
                        .id("outside")
                        .track_focus(&self.outside.clone().tab_stop(true)),
                )
                .child(
                    div()
                        .id("dialog")
                        .child(
                            div()
                                .id("first")
                                .track_focus(&self.first.clone().tab_stop(true)),
                        )
                        .child(
                            div()
                                .id("second")
                                .track_focus(&self.second.clone().tab_stop(true)),
                        )
                        .focus_trap("trap", &self.trap),
                )
        }
    }

    /// HS-17's mechanism: Tab and Shift+Tab, pressed more times than there are controls, never
    /// leave the trap.
    #[gpui::test]
    fn tab_and_shift_tab_stay_inside(cx: &mut TestAppContext) {
        cx.update(gpui_base::init);
        let (view, cx) = cx.add_window_view(|_, cx| Trapped {
            outside: cx.focus_handle(),
            trap: cx.focus_handle(),
            first: cx.focus_handle(),
            second: cx.focus_handle(),
        });
        cx.update(|window, cx| {
            let first = view.read(cx).first.clone();
            window.focus(&first, cx);
        });
        for keys in ["tab", "shift-tab"] {
            for _ in 0..5 {
                cx.simulate_keystrokes(keys);
                cx.update(|window, cx| {
                    let view = view.read(cx);
                    assert!(
                        view.first.is_focused(window) || view.second.is_focused(window),
                        "{keys} left the trap"
                    );
                    assert!(!view.outside.is_focused(window));
                });
            }
        }
    }
}

/// US1: the window, its frames and their status, reload, and the chrome kept uncovered.
mod window {
    use crate::common::World;
    use gpui::{Bounds, Entity, Pixels, TestAppContext, VisualTestContext, point, px, size};
    use host::dashboard::Dashboard;
    use host::plugins::FrameStatus;
    use host::text::Msg;

    fn statuses(
        view: &Entity<Dashboard>,
        cx: &mut VisualTestContext,
    ) -> Vec<(String, FrameStatus)> {
        view.read_with(cx, |view, _| {
            view.widgets()
                .iter()
                .map(|widget| (widget.label.as_str().to_owned(), widget.status))
                .collect()
        })
    }

    /// FR-007 to FR-009: one frame per approval, labelled with the id's last segment, with the
    /// host's status: rendered, or the host's words for the error kind; a failure does not stop
    /// the others.
    #[gpui::test]
    fn frames_show_each_widgets_status(cx: &mut TestAppContext) {
        let world = World::new();
        for name in ["hello-widget", "out-of-bounds", "window-error-imitation"] {
            world.approve(name);
        }
        let (view, cx) = world.open(cx);
        assert_eq!(
            statuses(&view, cx),
            vec![
                ("hello-widget".to_owned(), FrameStatus::Rendered),
                (
                    "out-of-bounds".to_owned(),
                    FrameStatus::Failed(Msg::ReasonCrashed)
                ),
                (
                    "window-error-imitation".to_owned(),
                    FrameStatus::Failed(Msg::ReasonReported)
                ),
            ]
        );
    }

    /// FR-005: the window focuses its root, and the first Tab reaches the first control.
    #[gpui::test]
    fn the_first_tab_reaches_the_first_control(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("hello-widget");
        let (view, cx) = world.open(cx);
        cx.update(|window, cx| assert!(view.read(cx).root_focus().is_focused(window)));
        cx.simulate_keystrokes("tab");
        cx.update(|window, cx| assert!(view.read(cx).reload_focus().is_focused(window)));
    }

    /// FR-040: "Reload" adds newly approved plugins and drops removed ones.
    #[gpui::test]
    fn reload_follows_the_record(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("hello-widget");
        let (view, cx) = world.open(cx);
        assert_eq!(statuses(&view, cx).len(), 1);
        world.approve("state-leak");
        view.update(cx, |view, cx| view.reload(cx));
        cx.run_until_parked();
        assert_eq!(statuses(&view, cx).len(), 2);
        let removed = runtime::manifest::PluginId::parse("local/hello-widget").expect("an id");
        world.grants.remove(&removed).expect("removed");
        view.update(cx, |view, cx| view.reload(cx));
        cx.run_until_parked();
        assert_eq!(
            statuses(&view, cx),
            vec![("state-leak".to_owned(), FrameStatus::Rendered)]
        );
    }

    fn intersects(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
        a.intersects(&b)
    }

    /// HS-01, FR-011: no frame intersects the chrome, at three window sizes and three scale
    /// factors, with the content scrolled so frames sit past the top and bottom.
    #[gpui::test]
    fn the_chrome_is_never_covered(cx: &mut TestAppContext) {
        let world = World::new();
        for name in [
            "hello-widget",
            "state-leak",
            "log-flood",
            "log-spoof",
            "out-of-bounds",
            "window-addresses",
            "window-error-imitation",
            "malformed-output",
        ] {
            world.approve(name);
        }
        let (view, cx) = world.open(cx);
        let frames = statuses(&view, cx).len();
        for (width, height) in [(800.0, 600.0), (1280.0, 800.0), (2560.0, 1440.0)] {
            for scale in [1.0, 1.5, 2.0] {
                cx.simulate_scale_factor_change(scale);
                cx.simulate_resize(size(px(width), px(height)));
                for offset in [0.0, -150.0, -400.0, -2000.0, 150.0] {
                    view.update(cx, |view, _| {
                        view.scroll_handle().set_offset(point(px(0.0), px(offset)))
                    });
                    cx.run_until_parked();
                    let chrome = cx.debug_bounds("chrome").expect("the chrome is drawn");
                    let content = cx.debug_bounds("content").expect("the content area");
                    assert!(
                        content.top() >= chrome.bottom(),
                        "the content area starts under the chrome"
                    );
                    for index in 0..frames {
                        let selector: &'static str =
                            Box::leak(format!("frame-{index}").into_boxed_str());
                        let Some(frame) = cx.debug_bounds(selector) else {
                            continue;
                        };
                        // What is drawn of a frame is what the content area, which clips, lets show
                        if let Some(visible) = Some(frame.intersect(&content)).filter(|visible| {
                            visible.size.width > px(0.0) && visible.size.height > px(0.0)
                        }) {
                            assert!(
                                !intersects(visible, chrome),
                                "frame {index} over the chrome at {width}x{height}, scale {scale}, offset {offset}"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// US2: a frame's menu and its panel, attached, never over the chrome, closing on Escape and on a
/// click outside that still reaches its target.
mod panel {
    use crate::common::World;
    use gpui::{Modifiers, TestAppContext, point, px, size};

    /// FR-010, FR-011: the panel is attached to its frame and never intersects the chrome.
    #[gpui::test]
    fn the_panel_is_attached_and_under_the_chrome(cx: &mut TestAppContext) {
        let world = World::new();
        for name in ["hello-widget", "state-leak", "log-flood", "log-spoof"] {
            world.approve(name);
        }
        let (view, cx) = world.open(cx);
        cx.update(|window, cx| view.update(cx, |view, cx| view.open_panel_of(1, window, cx)));
        cx.run_until_parked();
        for (width, height) in [(800.0, 600.0), (1280.0, 800.0), (2560.0, 1440.0)] {
            for scale in [1.0, 1.5, 2.0] {
                cx.simulate_scale_factor_change(scale);
                cx.simulate_resize(size(px(width), px(height)));
                for offset in [0.0, -200.0, -1000.0] {
                    view.update(cx, |view, _| {
                        view.scroll_handle().set_offset(point(px(0.0), px(offset)))
                    });
                    cx.run_until_parked();
                    let chrome = cx.debug_bounds("chrome").expect("the chrome");
                    let content = cx.debug_bounds("content").expect("the content area");
                    let frame = cx.debug_bounds("frame-1").expect("the frame");
                    let panel = cx.debug_bounds("panel-1").expect("the panel");
                    // Attached: beside the frame, or under it when the row has no room
                    let beside = (panel.left() - frame.right()).abs() < px(0.5)
                        && (panel.top() - frame.top()).abs() < px(0.5);
                    let under = (panel.top() - frame.bottom()).abs() < px(0.5)
                        && (panel.left() - frame.left()).abs() < px(0.5);
                    assert!(
                        beside || under,
                        "attached at {width}x{height} scale {scale}"
                    );
                    let visible = panel.intersect(&content);
                    if visible.size.width > px(0.0) && visible.size.height > px(0.0) {
                        assert!(
                            !visible.intersects(&chrome),
                            "panel over the chrome at {width}x{height} scale {scale} offset {offset}"
                        );
                    }
                }
            }
        }
    }

    /// FR-010: Escape closes the panel, and focus goes back to the frame's menu button.
    #[gpui::test]
    fn escape_closes_the_panel(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("hello-widget");
        let (view, cx) = world.open(cx);
        cx.update(|window, cx| view.update(cx, |view, cx| view.toggle_menu(0, window, cx)));
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(view.read(cx).settings_item_focus().is_focused(window));
        });
        cx.simulate_keystrokes("enter");
        cx.update(|window, cx| {
            let view = view.read(cx);
            assert_eq!(
                view.open_panel(),
                Some(0),
                "Enter on Settings opened the panel"
            );
            assert!(view.panel_close_focus().is_focused(window));
        });
        cx.simulate_keystrokes("escape");
        cx.update(|window, cx| {
            let view = view.read(cx);
            assert_eq!(view.open_panel(), None);
            assert!(
                view.menu_focus(0)
                    .expect("a menu button")
                    .is_focused(window)
            );
        });
    }

    /// FR-010: a click outside closes the panel and still reaches what it was aimed at: the rest
    /// of the window takes input while the panel is open.
    #[gpui::test]
    fn a_click_outside_closes_the_panel_and_reaches_its_target(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("hello-widget");
        let (view, cx) = world.open(cx);
        cx.update(|window, cx| view.update(cx, |view, cx| view.open_panel_of(0, window, cx)));
        cx.run_until_parked();
        let reloads = view.read_with(cx, |view, _| view.reloads());
        let reload = cx.debug_bounds("reload").expect("the Reload control");
        cx.simulate_click(reload.center(), Modifiers::none());
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert_eq!(view.open_panel(), None, "the panel closed");
            assert_eq!(view.reloads(), reloads + 1, "the click reached Reload");
        });
    }
}

/// US3: the host dialog's form (HS-02, HS-10, FR-016, FR-018, FR-040) and "Remove from
/// dashboard" (FR-038).
mod dialog_form {
    use crate::common::{World, request};
    use futures::channel::oneshot;
    use gpui::{Modifiers, TestAppContext, VisualTestContext};
    use host::dialog::{self, Answer};
    use std::time::Duration;

    fn answer_now(receiver: &mut oneshot::Receiver<Answer>) -> Option<Answer> {
        receiver.try_recv().ok().flatten()
    }

    /// HS-02: in every host window the scrim covers the whole window, chrome included; the
    /// dialog is drawn only in the window that asked, inside the scrim.
    #[gpui::test]
    fn the_scrim_covers_every_window_and_the_chrome(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("hello-widget");
        let (windows, _clock) = world.open_windows(cx, 2);
        let _answer = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        for (index, handle) in windows.iter().enumerate() {
            let mut cx = VisualTestContext::from_window((*handle).into(), cx);
            cx.run_until_parked();
            let size = cx.update(|window, _| window.viewport_size());
            let scrim = cx.debug_bounds("scrim").expect("the scrim is drawn");
            let chrome = cx.debug_bounds("chrome").expect("the chrome");
            assert_eq!(scrim.origin, gpui::point(gpui::px(0.0), gpui::px(0.0)));
            assert_eq!(scrim.size, size, "the scrim covers window {index} whole");
            let covers = |inner: gpui::Bounds<gpui::Pixels>| {
                inner.left() >= scrim.left()
                    && inner.top() >= scrim.top()
                    && inner.right() <= scrim.right()
                    && inner.bottom() <= scrim.bottom()
            };
            assert!(covers(chrome), "the scrim covers the chrome");
            let dialog = cx.debug_bounds("dialog");
            if index == 0 {
                let dialog = dialog.expect("the dialog in the window that asked");
                assert!(covers(dialog), "the dialog sits inside the scrim");
            } else {
                assert!(dialog.is_none(), "only the scrim in another window");
            }
        }
    }

    /// HS-10, FR-016: nothing outside the dialog takes input: a click on Reload or on the scrim,
    /// keys in another window, even after the delay; the other window has no focus.
    #[gpui::test]
    fn nothing_outside_the_dialog_takes_input(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("hello-widget");
        let (windows, clock) = world.open_windows(cx, 2);
        let mut answer = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        cx.run_until_parked();
        clock.advance(Duration::from_millis(1500));
        {
            let mut first = VisualTestContext::from_window(windows[0].into(), cx);
            first.run_until_parked();
            let reloads = windows[0]
                .read_with(&first, |view, _| view.reloads())
                .unwrap();
            let reload = first.debug_bounds("reload").expect("Reload");
            first.simulate_click(reload.center(), Modifiers::none());
            let scrim = first.debug_bounds("scrim").expect("the scrim");
            first.simulate_click(
                scrim.origin + gpui::point(gpui::px(5.0), gpui::px(5.0)),
                Modifiers::none(),
            );
            assert_eq!(
                windows[0]
                    .read_with(&first, |view, _| view.reloads())
                    .unwrap(),
                reloads,
                "the click on Reload did not reach it"
            );
        }
        {
            let mut second = VisualTestContext::from_window(windows[1].into(), cx);
            second.run_until_parked();
            second.update(|window, cx| assert!(window.focused(cx).is_none()));
            second.simulate_keystrokes("enter tab space escape");
        }
        assert_eq!(answer_now(&mut answer), None, "still open");
        assert!(cx.update(|cx| dialog::is_open(cx)));
    }

    /// HS-10: closing a window answers the dialog as a refusal.
    #[gpui::test]
    fn closing_a_window_refuses(cx: &mut TestAppContext) {
        let world = World::new();
        let (windows, _clock) = world.open_windows(cx, 2);
        let mut answer = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        cx.run_until_parked();
        let mut second = VisualTestContext::from_window(windows[1].into(), cx);
        assert!(second.simulate_close());
        assert_eq!(answer_now(&mut answer), Some(Answer::Refused));
    }

    /// FR-018: one dialog at a time; the next shows when the first is answered.
    #[gpui::test]
    fn dialogs_wait_in_order(cx: &mut TestAppContext) {
        let world = World::new();
        let (windows, _clock) = world.open_windows(cx, 1);
        let mut first = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        let mut second = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        let generation = cx.update(|cx| dialog::open(cx).map(|open| open.generation));
        cx.update(|cx| dialog::answer(cx, Answer::Confirmed));
        assert_eq!(answer_now(&mut first), Some(Answer::Confirmed));
        assert_eq!(answer_now(&mut second), None);
        let next = cx.update(|cx| dialog::open(cx).map(|open| open.generation));
        assert_eq!(next, generation.map(|g| g + 1), "the second is shown");
        cx.update(|cx| dialog::answer(cx, Answer::Refused));
        assert_eq!(answer_now(&mut second), Some(Answer::Refused));
        assert!(!cx.update(|cx| dialog::is_open(cx)));
    }

    /// FR-040: a reload asked for while a dialog is open waits for its answer.
    #[gpui::test]
    fn a_reload_waits_for_the_dialog(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("hello-widget");
        let (windows, _clock) = world.open_windows(cx, 1);
        let reloads = windows[0].read_with(cx, |view, _| view.reloads()).unwrap();
        let _answer = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        windows[0]
            .update(cx, |view, _, cx| view.reload(cx))
            .unwrap();
        assert_eq!(
            windows[0].read_with(cx, |view, _| view.reloads()).unwrap(),
            reloads
        );
        cx.update(|cx| dialog::answer(cx, Answer::Refused));
        cx.run_until_parked();
        assert_eq!(
            windows[0].read_with(cx, |view, _| view.reloads()).unwrap(),
            reloads + 1
        );
    }

    /// FR-038: "Remove from dashboard": confirmed, the approval and kept data go and the frame
    /// leaves; refused, nothing changes; focus goes back to the menu button.
    #[gpui::test]
    fn remove_from_dashboard(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("hello-widget");
        world.approve("state-leak");
        let (windows, clock) = world.open_windows(cx, 1);
        let mut cx = VisualTestContext::from_window(windows[0].into(), cx);
        let view = windows[0];
        // Refused with Escape
        view.update(&mut cx, |view, window, cx| view.ask_remove(0, window, cx))
            .unwrap();
        cx.run_until_parked();
        clock.advance(Duration::from_millis(1001));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(&cx, |view, _| view.widgets().len()).unwrap(),
            2
        );
        let button = view
            .read_with(&cx, |view, _| view.menu_focus(0).cloned())
            .unwrap()
            .expect("a menu button");
        cx.update(|window, _| assert!(button.is_focused(window), "focus back on the menu button"));
        // Confirmed with a click on Remove
        view.update(&mut cx, |view, window, cx| view.ask_remove(0, window, cx))
            .unwrap();
        cx.run_until_parked();
        clock.advance(Duration::from_millis(1001));
        let confirm = cx.debug_bounds("dialog-confirm").expect("Remove");
        cx.simulate_click(confirm.center(), Modifiers::none());
        cx.run_until_parked();
        let left: Vec<String> = view
            .read_with(&cx, |view, _| {
                view.widgets()
                    .iter()
                    .map(|w| w.id.as_str().to_owned())
                    .collect()
            })
            .unwrap();
        assert_eq!(left, vec!["local/state-leak".to_owned()]);
        let approvals = world.grants.approvals().expect("readable");
        assert_eq!(approvals.len(), 1);
    }
}
