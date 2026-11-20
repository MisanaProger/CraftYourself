//! A minimal server-sent events reader: `data` payloads of complete events.

#[derive(Debug, Default)]
pub struct SseParser {
    /// Bytes of an unfinished line (a chunk may split a UTF-8 character).
    pending: Vec<u8>,
    /// `data` lines of the current event.
    data: Vec<String>,
}

impl SseParser {
    /// Feeds bytes; returns the payloads of the events they complete.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for &b in bytes {
            if b != b'\n' {
                self.pending.push(b);
                continue;
            }
            let mut line = String::from_utf8_lossy(&self.pending).into_owned();
            self.pending.clear();
            if line.ends_with('\r') {
                line.pop();
            }
            self.line(&line, &mut out);
        }
        out
    }

    /// The end of the stream: an event without its final blank line.
    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.pending.is_empty() {
            let line = String::from_utf8_lossy(&std::mem::take(&mut self.pending)).into_owned();
            self.line(line.trim_end_matches('\r'), &mut out);
        }
        self.line("", &mut out);
        out
    }

    fn line(&mut self, line: &str, out: &mut Vec<String>) {
        if line.is_empty() {
            if !self.data.is_empty() {
                out.push(std::mem::take(&mut self.data).join("\n"));
            }
        } else if let Some(rest) = line.strip_prefix("data:") {
            self.data
                .push(rest.strip_prefix(' ').unwrap_or(rest).to_owned());
        }
        // `event:`, `id:`, `retry:` and comments are not needed.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_across_chunks() {
        let stream =
            "event: x\r\ndata: {\"a\":\"п\"}\r\n\r\n: ping\n\ndata: one\ndata: two\n\ndata: [DONE]";
        let bytes = stream.as_bytes();
        for n in [1, 2, 5, bytes.len()] {
            let mut p = SseParser::default();
            let mut out = Vec::new();
            for chunk in bytes.chunks(n) {
                out.extend(p.push(chunk));
            }
            out.extend(p.finish());
            assert_eq!(out, ["{\"a\":\"п\"}", "one\ntwo", "[DONE]"], "chunk {n}");
        }
    }
}
