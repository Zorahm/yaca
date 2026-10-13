//! Parsing of `text/event-stream`.
//!
//! Our own, not a library: we need exactly one piece of the spec — accumulating
//! `data:` lines up to an empty line — and a dependency would bring its own
//! error model on top of ours.

/// Incremental decoder. Feed it chunks of bytes and take the finished payloads.
/// A chunk boundary in the middle of a line or of a UTF-8 sequence is survived.
#[derive(Debug, Default)]
pub struct SseDecoder {
    /// Raw bytes not yet assembled into a completed event.
    buf: Vec<u8>,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the content of the `data` field for each completed event.
    /// Comments (`:`) and service fields are discarded.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);

        let mut out = Vec::new();
        // An event ends with an empty line. Look for it without touching the tail.
        while let Some(end) = find_separator(&self.buf) {
            let (raw, rest) = self.buf.split_at(end.0);
            let block = String::from_utf8_lossy(raw).into_owned();
            self.buf = rest[end.1..].to_vec();

            if let Some(data) = extract_data(&block) {
                out.push(data);
            }
        }
        out
    }
}

/// Position and length of the event separator: `\n\n` or `\r\n\r\n`.
fn find_separator(buf: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + 1 < buf.len() {
        if buf[i] == b'\n' && buf[i + 1] == b'\n' {
            return Some((i, 2));
        }
        if i + 3 < buf.len()
            && buf[i] == b'\r'
            && buf[i + 1] == b'\n'
            && buf[i + 2] == b'\r'
            && buf[i + 3] == b'\n'
        {
            return Some((i, 4));
        }
        i += 1;
    }
    None
}

fn extract_data(block: &str) -> Option<String> {
    let mut data: Vec<&str> = Vec::new();
    for line in block.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let Some(rest) = line.strip_prefix("data:") else {
            continue;
        };
        data.push(rest.strip_prefix(' ').unwrap_or(rest));
    }
    if data.is_empty() {
        None
    } else {
        Some(data.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn отдаёт_событие_только_после_пустой_строки() {
        let mut d = SseDecoder::new();
        assert!(d.push(b"event: ping\ndata: {\"a\":1}").is_empty());
        assert_eq!(d.push(b"\n\n"), vec![r#"{"a":1}"#]);
    }

    #[test]
    fn несколько_событий_из_одного_чанка() {
        let mut d = SseDecoder::new();
        let got = d.push(b"data: one\n\ndata: two\n\n");
        assert_eq!(got, vec!["one", "two"]);
    }

    #[test]
    fn переживает_разрыв_посреди_utf8() {
        let mut d = SseDecoder::new();
        let payload = "data: привет\n\n".as_bytes();
        let (head, tail) = payload.split_at(9); // cut inside a multibyte character
        assert!(d.push(head).is_empty());
        assert_eq!(d.push(tail), vec!["привет"]);
    }

    #[test]
    fn понимает_crlf() {
        let mut d = SseDecoder::new();
        assert_eq!(d.push(b"data: x\r\n\r\n"), vec!["x"]);
    }

    #[test]
    fn комментарии_отбрасываются() {
        let mut d = SseDecoder::new();
        assert!(d.push(b": keep-alive\n\n").is_empty());
    }
}
