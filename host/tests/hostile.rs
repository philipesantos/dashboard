//! Hostile cases of the host window (FR-031; contracts/bans-and-checks.md): each one tries to
//! break a rule the window keeps, and passes only when the rule holds. Part of the security suite.

mod common;

/// FR-008, FR-027, FR-029: content of markup and addresses.
mod content {
    use crate::common::World;
    use gpui::TestAppContext;
    use gpui::http_client::FakeHttpClient;
    use host::plugins::FrameStatus;
    use host::text::Msg;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// FR-008, FR-027, FR-029 (hostile): content of markup and addresses renders, nothing of it
    /// is drawn, and drawing the window makes no request.
    #[gpui::test]
    fn hostile_content_is_never_drawn_or_fetched(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("window-addresses");
        let requests = Arc::new(AtomicUsize::new(0));
        let (view, cx) = world.open(cx);
        let counter = Arc::clone(&requests);
        cx.update(|_, cx| {
            cx.set_http_client(FakeHttpClient::create(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                async { anyhow::bail!("no request is expected") }
            }))
        });
        view.update(cx, |view, cx| view.reload(cx));
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |view, _| view
                .widgets()
                .iter()
                .map(|w| (w.label.as_str().to_owned(), w.status))
                .collect::<Vec<_>>()),
            vec![("window-addresses".to_owned(), FrameStatus::Rendered)]
        );
        assert_eq!(
            requests.load(Ordering::SeqCst),
            0,
            "no request left the window"
        );
    }

    /// FR-008: an error whose text imitates the host is shown only as the host's words for its
    /// kind.
    #[gpui::test]
    fn an_error_imitating_the_host_shows_only_the_hosts_words(cx: &mut TestAppContext) {
        let world = World::new();
        world.approve("window-error-imitation");
        let (view, cx) = world.open(cx);
        let status = view.read_with(cx, |view, _| view.widgets()[0].status);
        assert_eq!(status, FrameStatus::Failed(Msg::ReasonReported));
    }
}

/// HS-11, FR-017: input meant for something else never answers a new dialog.
mod gate {
    use crate::common::{World, request};
    use futures::channel::oneshot;
    use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};
    use host::dialog::{self, Answer};
    use std::time::Duration;

    fn answer_now(receiver: &mut oneshot::Receiver<Answer>) -> Option<Answer> {
        receiver.try_recv().ok().flatten()
    }

    /// A key pressed and released, as a real window sends it: GPUI's `simulate_keystrokes`
    /// sends only the press.
    fn press(cx: &mut VisualTestContext, key: &str) {
        cx.simulate_keystrokes(key);
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).expect("a key"),
        });
    }

    /// Keys at 0, 500 and 999 ms answer nothing; at 1000 ms one does.
    #[gpui::test]
    fn keys_before_a_second_answer_nothing(cx: &mut TestAppContext) {
        let world = World::new();
        let (windows, clock) = world.open_windows(cx, 1);
        let mut answer = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        let mut cx = VisualTestContext::from_window(windows[0].into(), cx);
        cx.run_until_parked();
        for (at, key) in [(0, "enter"), (500, "space"), (499, "escape")] {
            clock.advance(Duration::from_millis(at));
            press(&mut cx, key);
            assert_eq!(
                answer_now(&mut answer),
                None,
                "{key} answered before 1000 ms"
            );
        }
        clock.advance(Duration::from_millis(1));
        press(&mut cx, "enter");
        // Enter on the initial focus, Cancel, refuses: there is no default button (HS-16)
        assert_eq!(answer_now(&mut answer), Some(Answer::Refused));
    }

    /// A key down from before the dialog answers nothing until released and pressed again.
    #[gpui::test]
    fn a_key_held_from_before_answers_nothing(cx: &mut TestAppContext) {
        let world = World::new();
        let (windows, clock) = world.open_windows(cx, 1);
        cx.update(|cx| dialog::key_down(cx, "enter"));
        let mut answer = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        let mut cx = VisualTestContext::from_window(windows[0].into(), cx);
        cx.run_until_parked();
        clock.advance(Duration::from_millis(2000));
        // This press is the held key's own: its key-up releases it
        press(&mut cx, "enter");
        assert_eq!(answer_now(&mut answer), None, "the held key answered");
        press(&mut cx, "enter");
        assert_eq!(answer_now(&mut answer), Some(Answer::Refused));
    }

    /// A pointer press that began before the gate opened answers nothing; a whole click after
    /// does.
    #[gpui::test]
    fn a_click_begun_before_the_delay_answers_nothing(cx: &mut TestAppContext) {
        let world = World::new();
        let (windows, clock) = world.open_windows(cx, 1);
        let mut answer = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        let mut cx = VisualTestContext::from_window(windows[0].into(), cx);
        cx.run_until_parked();
        let confirm = cx.debug_bounds("dialog-confirm").expect("Remove").center();
        cx.simulate_click(confirm, Modifiers::none());
        assert_eq!(answer_now(&mut answer), None, "a click at 0 ms answered");
        clock.advance(Duration::from_millis(900));
        cx.simulate_mouse_down(confirm, MouseButton::Left, Modifiers::none());
        clock.advance(Duration::from_millis(200));
        cx.simulate_mouse_up(confirm, MouseButton::Left, Modifiers::none());
        assert_eq!(
            answer_now(&mut answer),
            None,
            "a press begun at 900 ms answered"
        );
        cx.simulate_click(confirm, Modifiers::none());
        assert_eq!(answer_now(&mut answer), Some(Answer::Confirmed));
    }

    /// The delay starts over when the window regains focus and when a new dialog replaces the
    /// first.
    #[gpui::test]
    fn the_delay_starts_over(cx: &mut TestAppContext) {
        let world = World::new();
        let (windows, clock) = world.open_windows(cx, 1);
        let mut first = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        let mut second = cx.update(|cx| dialog::ask(cx, windows[0].into(), request()));
        let mut cx = VisualTestContext::from_window(windows[0].into(), cx);
        cx.run_until_parked();
        clock.advance(Duration::from_millis(900));
        cx.deactivate_window();
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        clock.advance(Duration::from_millis(200));
        press(&mut cx, "enter");
        assert_eq!(
            answer_now(&mut first),
            None,
            "the refocus did not restart the delay"
        );
        clock.advance(Duration::from_millis(800));
        press(&mut cx, "enter");
        assert_eq!(answer_now(&mut first), Some(Answer::Refused));
        // The second dialog starts its own delay
        cx.run_until_parked();
        press(&mut cx, "enter");
        assert_eq!(
            answer_now(&mut second),
            None,
            "the next dialog answered at once"
        );
        clock.advance(Duration::from_millis(1000));
        press(&mut cx, "enter");
        assert_eq!(answer_now(&mut second), Some(Answer::Refused));
    }
}

/// FR-041: a plugin that declares a local connector never loads in the window, which runs the
/// runtime without development mode; its frame says so in the host's words.
mod local_connector {
    use crate::common::{World, fixture};
    use gpui::TestAppContext;
    use host::plugins::FrameStatus;
    use host::text::Msg;
    use runtime::manifest::Manifest;
    use serde_json::json;

    #[gpui::test]
    fn a_local_connector_never_loads_in_the_window(cx: &mut TestAppContext) {
        let world = World::new();
        let manifest = json!({
            "manifest_version": 1, "id": "local/window-local-connector", "version": "0.1.0",
            "author": "Dashboard contributors", "permissions": [],
            "connectors": [
                { "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true }
            ]
        })
        .to_string();
        let component = std::fs::read(fixture("net-probe")).expect("the bare component");
        let package =
            runtime::package::pack(&component, manifest.as_bytes()).expect("a valid package");
        let path = world.dir.path().join("window-local-connector.wasm");
        std::fs::write(&path, &package).expect("written");
        let origin = path.to_string_lossy().into_owned();
        // Approved as the record would hold it; the window must still refuse the local connector
        let parsed = Manifest::parse(manifest.as_bytes()).expect("a valid manifest");
        world
            .grants
            .approve(
                &parsed,
                &origin,
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("recorded");
        let (view, cx) = world.open(cx);
        let status = view.read_with(cx, |view, _| view.widgets()[0].status);
        assert!(
            matches!(
                status,
                FrameStatus::Failed(Msg::ReasonConnector | Msg::ReasonApproval)
            ),
            "{status:?}"
        );
    }
}

/// FR-030: the window's HTTP client refuses every request, remote or `file://`.
mod network {
    use gpui::TestAppContext;

    #[gpui::test]
    async fn the_http_client_refuses_every_address(cx: &mut TestAppContext) {
        cx.update(|cx| host::app::install(cx).expect("installs"));
        let client = cx.update(|cx| cx.http_client());
        for address in [
            "https://attacker.example/pixel.png",
            "http://127.0.0.1:9/",
            "file:///etc/passwd",
        ] {
            let answer = client.get(address, Default::default(), false).await;
            assert!(answer.is_err(), "{address} was not refused");
        }
    }
}

/// HS-12, FR-019: a prompt GPUI is asked for never shows; the host's builder refuses it at once.
mod prompts {
    use gpui::{PromptLevel, TestAppContext};
    use host::app::REFUSED_PROMPT;

    struct Empty;
    impl gpui::Render for Empty {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    #[gpui::test]
    async fn a_native_prompt_is_refused(cx: &mut TestAppContext) {
        cx.update(|cx| host::app::install(cx).expect("installs"));
        let window = cx.add_window(|_, _| Empty);
        // The call clippy bans everywhere in the host: here to prove the builder refuses it
        #[allow(clippy::disallowed_methods)]
        let answer = window
            .update(cx, |_, window, cx| {
                window.prompt(PromptLevel::Critical, "Allow access?", None, &["Allow"], cx)
            })
            .expect("the window");
        cx.run_until_parked();
        assert_eq!(answer.await.ok(), Some(REFUSED_PROMPT));
    }
}
