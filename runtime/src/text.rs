//! Text that comes from a plugin, made safe to show: it can neither pass for the host's own
//! output (FR-011) nor grow without bound.

/// Follows text that was cut; the byte limit does not count it.
const TRUNCATED: &str = " [truncated]";

/// Escapes `text` and cuts it to `max_bytes`.
///
/// Every control character (`char::is_control`: line breaks, escape, DEL, the C1 controls) and
/// every bidirectional-text control is shown as its Rust escape (`\n`, `\u{1b}`, `\u{202e}`), so
/// it cannot start a line, drive the terminal or reorder what follows it on screen. Every other
/// character is kept. A longer result is cut on a character boundary and followed by
/// ` [truncated]`.
pub(crate) fn untrusted(text: &str, max_bytes: usize) -> String {
    let mut escaped = String::with_capacity(text.len().min(max_bytes));
    for c in text.chars() {
        if needs_escape(c) {
            escaped.extend(c.escape_default());
        } else {
            escaped.push(c);
        }
        // Past the limit the rest would be cut anyway; stop here so a huge text costs little.
        if escaped.len() > max_bytes {
            break;
        }
    }
    if escaped.len() > max_bytes {
        let mut cut = max_bytes;
        while !escaped.is_char_boundary(cut) {
            cut -= 1;
        }
        escaped.truncate(cut);
        escaped.push_str(TRUNCATED);
    }
    escaped
}

/// Escapes text that came from a package, or from a record a person can edit, before it is shown,
/// as [`untrusted`] does, without cutting it.
pub fn escape(text: &str) -> String {
    untrusted(text, usize::MAX)
}

/// A stored name or value, as `runtime storage show` prints it
/// (`specs/004-plugin-storage/contracts/cli.md`): UTF-8 in double quotes, escaped as
/// [`untrusted`] escapes, with `"` as `\"` and `\` as `\\` inside the quotes, so no entry can
/// pass for two fields; other bytes as `bytes(<size>) <hexadecimal>`. Only the first `max_bytes`
/// of `bytes` are shown, followed by ` [truncated, <size> bytes]`.
pub fn quote_entry(bytes: &[u8], max_bytes: usize) -> String {
    let _ = (bytes, max_bytes);
    String::new()
}

/// Control characters, and the characters that change the direction of the text after them.
fn needs_escape(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escape(text: &str) -> String {
        untrusted(text, usize::MAX)
    }

    #[test]
    fn keeps_ordinary_text_unchanged() {
        assert_eq!(escape(""), "");
        assert_eq!(
            escape("Olá, 世界! \\n stays as typed"),
            "Olá, 世界! \\n stays as typed"
        );
    }

    #[test]
    fn escapes_c0_controls_and_del() {
        assert_eq!(
            escape("a\nb\rc\td\u{1b}[2Ke\u{7}\u{0}\u{7f}"),
            r"a\nb\rc\td\u{1b}[2Ke\u{7}\u{0}\u{7f}"
        );
    }

    #[test]
    fn escapes_c1_controls() {
        // U+009B is the one-character form of the terminal's control sequence introducer
        assert_eq!(escape("\u{9b}31m\u{85}"), r"\u{9b}31m\u{85}");
    }

    #[test]
    fn escapes_bidirectional_controls() {
        assert_eq!(
            escape("\u{202e}dlrow\u{202c}\u{2066}\u{2069}\u{200f}\u{61c}"),
            r"\u{202e}dlrow\u{202c}\u{2066}\u{2069}\u{200f}\u{61c}"
        );
    }

    #[test]
    fn keeps_text_at_the_limit_whole() {
        assert_eq!(untrusted("abcd", 4), "abcd");
    }

    #[test]
    fn cuts_longer_text_and_marks_it() {
        assert_eq!(untrusted("abcdef", 4), "abcd [truncated]");
    }

    #[test]
    fn cuts_on_a_character_boundary() {
        // "€" takes 3 bytes: a cut at 4 bytes falls inside the second one
        assert_eq!(untrusted("€€€", 4), "€ [truncated]");
    }

    #[test]
    fn quotes_text_entries() {
        assert_eq!(quote_entry(b"count", 1024), r#""count""#);
        assert_eq!(quote_entry(b"", 1024), r#""""#);
        assert_eq!(
            quote_entry("notes/0 olá".as_bytes(), 1024),
            r#""notes/0 olá""#
        );
    }

    #[test]
    fn escapes_quotes_and_backslashes_inside_the_quotes() {
        // Without the escapes this would read as two fields: "count" = "999"
        assert_eq!(
            quote_entry(br#"count" = "999"#, 1024),
            r#""count\" = \"999""#
        );
        // A backslash and an n stay apart from a line break
        assert_eq!(quote_entry(br"a\nb", 1024), r#""a\\nb""#);
        assert_eq!(quote_entry(b"a\nb", 1024), r#""a\nb""#);
    }

    #[test]
    fn never_lets_line_breaks_or_terminal_escapes_through() {
        let quoted = quote_entry(b"x\r\n\x1b[2J\x1b]0;title\x07\x7f\xc2\x9b31m", 1024);
        assert!(
            !quoted.chars().any(needs_escape),
            "a control character reached the output: {quoted:?}"
        );
        assert_eq!(
            quoted,
            r#""x\r\n\u{1b}[2J\u{1b}]0;title\u{7}\u{7f}\u{9b}31m""#
        );
        let bidi = quote_entry("\u{202e}evil".as_bytes(), 1024);
        assert_eq!(bidi, r#""\u{202e}evil""#);
    }

    #[test]
    fn shows_bytes_that_are_not_text_in_hexadecimal() {
        assert_eq!(
            quote_entry(&[0x00, 0xff, 0x10, 0xa0], 1024),
            "bytes(4) 00ff10a0"
        );
        // A NUL is text, escaped; an invalid UTF-8 sequence is not
        assert_eq!(quote_entry(b"\0", 1024), r#""\u{0}""#);
        assert_eq!(quote_entry(b"ok\xff", 1024), "bytes(3) 6f6bff");
    }

    #[test]
    fn cuts_long_entries_and_names_their_size() {
        let long = "a".repeat(2000);
        assert_eq!(
            quote_entry(long.as_bytes(), 1024),
            format!("\"{}\" [truncated, 2000 bytes]", "a".repeat(1024))
        );
        assert_eq!(
            quote_entry(&[0xff; 1500], 4),
            "bytes(1500) ffffffff [truncated, 1500 bytes]"
        );
        // On a character boundary: "€" takes 3 bytes
        assert_eq!(
            quote_entry("€€€".as_bytes(), 4),
            r#""€" [truncated, 9 bytes]"#
        );
        // Exactly at the limit is whole
        assert_eq!(quote_entry(b"abcd", 4), r#""abcd""#);
    }

    #[test]
    fn counts_escapes_against_the_limit() {
        assert_eq!(untrusted("\n\n\n", 4), r"\n\n [truncated]");
    }
}
