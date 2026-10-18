//! A fetched body turned into text the model can read.
//!
//! Split off from `web_fetch` because nothing here touches a socket: this is a
//! pure function of bytes plus a `Content-Type`, which is what lets it be
//! tested without a server at all. What is left in the tool is the network.

use crate::error::ToolError;

/// How much of a page the model is given.
///
/// Twice what a command gets: a build log repeats itself, a documentation page
/// does not, and a page cut in half is a page the model will fetch again.
pub(crate) const MODEL_OUTPUT_MAX_BYTES: usize = 60_000;

/// Tags whose text is furniture rather than content.
///
/// Dropped before conversion, not after: once a sidebar is Markdown it is
/// indistinguishable from the article, and the model pays for it in tokens.
const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "nav", "header", "footer", "aside", "form", "svg", "template",
];

/// A body that turned out not to be text at all.
const NOT_TEXT: &str = "not text";

#[derive(Debug)]
pub(crate) struct Rendered {
    /// Markdown for HTML, the body as it came for everything else. Already cut
    /// to [`MODEL_OUTPUT_MAX_BYTES`]; the feed and the model see this same
    /// string, and only the untrusted-content header on top differs.
    pub text: String,
    /// Characters the page had before the cut.
    pub chars: usize,
    pub truncated: bool,
}

/// The media type without its parameters: `text/html; charset=utf-8` → `text/html`.
pub(crate) fn mime(content_type: &str) -> &str {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches('/')
}

enum Kind {
    Html,
    Text,
}

/// What to do with this media type, or `None` if there is nothing to do.
///
/// An empty type is sniffed rather than refused: servers that hand-roll their
/// responses leave the header off, and refusing a page that is plainly HTML
/// would be a lie about why it failed.
fn kind_of(content_type: &str, body: &[u8]) -> Option<Kind> {
    let mime = mime(content_type).to_ascii_lowercase();

    if mime.is_empty() {
        return Some(if looks_like_html(body) {
            Kind::Html
        } else {
            Kind::Text
        });
    }

    if mime == "text/html" || mime == "application/xhtml+xml" {
        return Some(Kind::Html);
    }

    let textual = mime.starts_with("text/")
        || mime.ends_with("+json")
        || mime.ends_with("+xml")
        || matches!(
            mime.as_str(),
            "application/json" | "application/xml" | "application/javascript"
        );

    textual.then_some(Kind::Text)
}

fn looks_like_html(body: &[u8]) -> bool {
    let head = &body[..body.len().min(1024)];
    let head = String::from_utf8_lossy(head).to_ascii_lowercase();
    let head = head.trim_start();
    head.starts_with("<!doctype html") || head.starts_with("<html")
}

/// Bytes to a `String` using the charset the server declared.
///
/// The header is believed and `<meta charset>` is not read: a page that
/// disagrees with its own server is rare, and sniffing the document would mean
/// parsing it twice. Anything unrecognised falls back to UTF-8, which is what
/// the web is.
fn decode(bytes: &[u8], content_type: &str) -> String {
    let label = content_type
        .split(';')
        .skip(1)
        .filter_map(|param| param.split_once('='))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case("charset"))
        .map(|(_, value)| value.trim().trim_matches('"'));

    let encoding = label
        .and_then(|l| encoding_rs::Encoding::for_label(l.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);

    encoding.decode(bytes).0.into_owned()
}

/// The whole pipeline: decode, convert, cut.
pub(crate) fn render(bytes: &[u8], content_type: &str) -> Result<Rendered, ToolError> {
    let kind = kind_of(content_type, bytes)
        .ok_or_else(|| ToolError::failed(format!("{NOT_TEXT}: {}", mime(content_type))))?;

    let decoded = decode(bytes, content_type);

    let text = match kind {
        Kind::Text => decoded,
        Kind::Html => htmd::HtmlToMarkdown::builder()
            .skip_tags(SKIP_TAGS.to_vec())
            .build()
            .convert(&decoded)
            .map_err(|e| ToolError::failed(format!("failed to parse HTML: {e}")))?,
    };
    let text = text.trim().to_string();

    Ok(cut(text))
}

/// Head of the page plus a note about what was dropped.
///
/// Head and not head-and-tail the way command output is cut: a log says what
/// happened last, a page says what it is about first, and its tail is a
/// colophon.
fn cut(text: String) -> Rendered {
    let chars = text.chars().count();
    if text.len() <= MODEL_OUTPUT_MAX_BYTES {
        return Rendered {
            text,
            chars,
            truncated: false,
        };
    }

    let mut end = MODEL_OUTPUT_MAX_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    // Back off to a line break so the cut does not land inside a fenced block
    // or a table row, where half a line reads as broken Markdown rather than
    // as a page that continues.
    if let Some(newline) = text[..end].rfind('\n') {
        end = newline;
    }

    let head = &text[..end];
    let shown = head.chars().count();
    Rendered {
        text: format!("{head}\n\n[… page truncated: showing {shown} of {chars} characters …]"),
        chars,
        truncated: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(body: &str) -> Rendered {
        render(body.as_bytes(), "text/html; charset=utf-8").unwrap()
    }

    #[test]
    fn furniture_does_not_reach_the_model() {
        let page = html(
            "<html><body>\
             <nav><a href='/x'>Home</a></nav>\
             <script>alert(1)</script>\
             <style>body{color:red}</style>\
             <h1>Title</h1><p>Article body.</p>\
             <footer>© 2026</footer>\
             </body></html>",
        );

        assert!(page.text.contains("# Title"));
        assert!(page.text.contains("Article body."));
        for gone in ["Home", "alert", "color:red", "© 2026"] {
            assert!(!page.text.contains(gone), "{gone} remained in text");
        }
    }

    #[test]
    fn code_blocks_survive_the_conversion() {
        // The whole reason for fetching documentation.
        let page = html("<pre><code class=\"language-rust\">let x = 1;</code></pre>");
        assert!(page.text.contains("let x = 1;"), "{}", page.text);
        assert!(page.text.contains("```"), "{}", page.text);
    }

    #[test]
    fn json_comes_back_untouched() {
        let body = br#"{"name":"htmd","version":"0.5.5"}"#;
        let page = render(body, "application/json").unwrap();
        assert_eq!(page.text, r#"{"name":"htmd","version":"0.5.5"}"#);
        assert!(!page.truncated);
    }

    #[test]
    fn a_declared_charset_is_honoured() {
        // "Café" in windows-1252. Decoded as UTF-8 it would be mojibake, and
        // mojibake in the feed is the interface lying about the page (§0).
        let body: &[u8] = &[b'C', b'a', b'f', 0xE9];
        let page = render(body, "text/plain; charset=windows-1252").unwrap();
        assert_eq!(page.text, "Café");
    }

    #[test]
    fn an_unknown_charset_falls_back_to_utf8() {
        let page = render("Hello".as_bytes(), "text/plain; charset=x-made-up").unwrap();
        assert_eq!(page.text, "Hello");
    }

    #[test]
    fn binary_is_refused_by_its_type() {
        let err = render(&[0x89, b'P', b'N', b'G'], "image/png").unwrap_err();
        assert!(err.to_string().contains(NOT_TEXT));
        assert!(err.to_string().contains("image/png"));

        // PDF is not supported either, and says so by its type rather than by
        // arriving as a page of binary noise.
        let err = render(b"%PDF-1.7", "application/pdf").unwrap_err();
        assert!(err.to_string().contains(NOT_TEXT));
    }

    #[test]
    fn a_missing_type_is_sniffed() {
        let page = render(b"<html><body><h1>Hi</h1></body></html>", "").unwrap();
        assert_eq!(page.text, "# Hi");

        let page = render(b"just words", "").unwrap();
        assert_eq!(page.text, "just words");
    }

    #[test]
    fn a_long_page_is_cut_at_a_line_with_a_note() {
        let line = "a line of text that takes up some space on each row\n";
        let body = line.repeat(4_000);
        let chars = body.trim().chars().count();
        assert!(body.len() > MODEL_OUTPUT_MAX_BYTES);

        let page = render(body.as_bytes(), "text/plain").unwrap();
        assert!(page.truncated);
        assert_eq!(page.chars, chars);
        assert!(page.text.len() < MODEL_OUTPUT_MAX_BYTES + 200);
        assert!(page.text.contains(&format!("of {chars} characters")));
        // The cut lands on a line break, not inside a word.
        let head = page.text.split("\n\n[…").next().unwrap();
        assert!(head.ends_with("row"), "cut in the middle of a line");
    }

    #[test]
    fn a_short_page_carries_no_note() {
        let page = render(b"two words", "text/plain").unwrap();
        assert!(!page.truncated);
        assert_eq!(page.chars, 9);
        assert!(!page.text.contains("truncated"));
    }

    #[test]
    fn the_media_type_loses_its_parameters() {
        assert_eq!(mime("text/html; charset=utf-8"), "text/html");
        assert_eq!(mime("text/html"), "text/html");
        assert_eq!(mime(""), "");
    }
}
