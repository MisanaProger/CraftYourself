//! What of a mentor answer may be shown while it is still being generated.
//!
//! The policy judges code and HTML pages on the whole answer
//! ([`crate::ResponseConstraints::check_answer`], `strip_pages`): a block
//! that is fine on its own may be removed because of what follows. Text
//! outside fenced blocks is never removed, so only that text is streamed;
//! a fenced block is withheld until the answer is complete and checked, and
//! the stored answer replaces the streamed draft. Fences are recognised
//! exactly as the policy recognises them: a line whose trimmed start is
//! ```` ``` ````.

/// A piece of a streamed answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Text outside fenced blocks.
    Text(String),
    /// A fenced block started; its content is withheld.
    Withheld,
}

#[derive(Debug, Default)]
pub struct ProseStream {
    line: String,
    /// Bytes of `line` already emitted.
    emitted: usize,
    inside: bool,
}

const FENCE: &str = "```";

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with(FENCE)
}

/// Whether a partial line outside a block may still turn into a fence.
fn may_become_fence(partial: &str) -> bool {
    let t = partial.trim_start();
    t.is_empty() || FENCE.starts_with(t) || t.starts_with(FENCE)
}

impl ProseStream {
    pub fn new() -> Self {
        Self::default()
    }

    fn text(out: &mut Vec<Piece>, s: &str) {
        if s.is_empty() {
            return;
        }
        match out.last_mut() {
            Some(Piece::Text(t)) => t.push_str(s),
            _ => out.push(Piece::Text(s.to_owned())),
        }
    }

    /// Feeds a chunk of the model's output; returns what may be shown now.
    pub fn push(&mut self, chunk: &str) -> Vec<Piece> {
        let mut out = Vec::new();
        for c in chunk.chars() {
            if c != '\n' {
                self.line.push(c);
                continue;
            }
            if is_fence(&self.line) {
                if !self.inside {
                    out.push(Piece::Withheld);
                }
                self.inside = !self.inside;
            } else if !self.inside {
                let rest = self.line[self.emitted..].to_owned();
                Self::text(&mut out, &rest);
                Self::text(&mut out, "\n");
            }
            self.line.clear();
            self.emitted = 0;
        }
        if !self.inside && !may_become_fence(&self.line) {
            let rest = self.line[self.emitted..].to_owned();
            Self::text(&mut out, &rest);
            self.emitted = self.line.len();
        }
        out
    }

    /// The end of the answer: flushes a last line without a newline.
    pub fn finish(&mut self) -> Vec<Piece> {
        let mut out = Vec::new();
        let line = std::mem::take(&mut self.line);
        if is_fence(&line) {
            if !self.inside {
                out.push(Piece::Withheld);
            }
        } else if !self.inside {
            Self::text(&mut out, &line[self.emitted..]);
        }
        self.emitted = 0;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `answer` in chunks of `n` characters and joins the shown text.
    fn shown(answer: &str, n: usize) -> (String, usize) {
        let mut s = ProseStream::new();
        let chars: Vec<char> = answer.chars().collect();
        let mut pieces = Vec::new();
        for chunk in chars.chunks(n) {
            pieces.extend(s.push(&chunk.iter().collect::<String>()));
        }
        pieces.extend(s.finish());
        let mut text = String::new();
        let mut withheld = 0;
        for p in pieces {
            match p {
                Piece::Text(t) => text.push_str(&t),
                Piece::Withheld => withheld += 1,
            }
        }
        (text, withheld)
    }

    #[test]
    fn code_and_pages_are_never_streamed() {
        let answer = "Look at `parse`.\n  ```rust\nlet secret = 1;\n```\nThen:\n```html\n<script>x</script>\n```\nWhy?";
        for n in [1, 2, 3, 7, 100] {
            let (text, withheld) = shown(answer, n);
            assert_eq!(text, "Look at `parse`.\nThen:\nWhy?", "chunk {n}");
            assert_eq!(withheld, 2);
            assert!(!text.contains("secret") && !text.contains("script"));
        }
    }

    #[test]
    fn prose_is_streamed_before_the_line_ends() {
        let mut s = ProseStream::new();
        assert_eq!(s.push("What did"), [Piece::Text("What did".into())]);
        assert_eq!(s.push(" you try"), [Piece::Text(" you try".into())]);
        // A line that may still become a fence is held back.
        assert_eq!(s.push("?\n``"), [Piece::Text("?\n".into())]);
        assert_eq!(s.push("`py\nx = 1"), [Piece::Withheld]);
        assert_eq!(s.finish(), [], "an unclosed block runs to the end");
        let mut s = ProseStream::new();
        assert_eq!(s.push("  "), []);
        assert_eq!(s.push("`x` is"), [Piece::Text("  `x` is".into())]);
    }

    #[test]
    fn shown_text_is_what_the_policy_keeps() {
        let answer = "Intro\n```\ncode\n```\nOutro\n";
        let kept = crate::constraints::strip_pages(answer);
        let (text, _) = shown(answer, 4);
        let prose: Vec<&str> = kept
            .lines()
            .filter(|l| !l.starts_with("```") && *l != "code")
            .collect();
        assert_eq!(text.trim_end(), prose.join("\n"));
    }
}
