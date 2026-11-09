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
