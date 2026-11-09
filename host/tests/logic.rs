//! The host's logic, headless: GPUI's test platform where a window is needed, plain functions
//! elsewhere (research R13, layer 1).

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
