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
    fn counts_escapes_against_the_limit() {
        assert_eq!(untrusted("\n\n\n", 4), r"\n\n [truncated]");
    }
}
