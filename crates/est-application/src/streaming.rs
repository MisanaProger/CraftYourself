//! Live mentor answers (stage 4): text is pushed to the interface while the
//! model writes it.
//!
//! Only what the policy can never remove is pushed — text outside fenced
//! blocks ([`est_ai_policy::stream`]); code and HTML pages are withheld
//! until the whole answer is checked. Deltas go to live clients only, not
//! to the event log: they are not observations, and the stored answer
//! (`conversation.message.created`) replaces the draft.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use est_ai_policy::stream::{Piece, ProseStream};
use est_core::{ConversationId, DomainEvent, MessageId, events};
use serde_json::json;

use crate::ports::EventPublisher;

/// Pushes at most this often, unless a line ends or the buffer fills.
const FLUSH_EVERY: Duration = Duration::from_millis(60);
const FLUSH_BYTES: usize = 48;

struct State {
    prose: ProseStream,
    buffer: String,
    last_flush: Instant,
    seq: u64,
}

pub(crate) struct AnswerStream {
    publisher: Arc<dyn EventPublisher>,
    conversation: ConversationId,
    in_reply_to: MessageId,
    state: Mutex<State>,
}

impl AnswerStream {
    /// Announces an answer to `in_reply_to`.
    pub fn start(
        publisher: Arc<dyn EventPublisher>,
        conversation: &ConversationId,
        in_reply_to: &MessageId,
        level: u8,
    ) -> Self {
        let s = Self {
            publisher,
            conversation: conversation.clone(),
            in_reply_to: in_reply_to.clone(),
            state: Mutex::new(State {
                prose: ProseStream::new(),
                buffer: String::new(),
                last_flush: Instant::now(),
                seq: 0,
            }),
        };
        s.publish(
            events::AI_ANSWER_STARTED,
            json!({ "in_reply_to": s.in_reply_to, "level": level }),
        );
        s
    }

    fn publish(&self, kind: &str, payload: serde_json::Value) {
        self.publisher
            .publish(&DomainEvent::new(kind, payload).conversation(&self.conversation));
    }

    fn flush(&self, st: &mut State) {
        if st.buffer.is_empty() {
            return;
        }
        st.seq += 1;
        let text = std::mem::take(&mut st.buffer);
        self.publish(
            events::AI_ANSWER_DELTA,
            json!({ "in_reply_to": self.in_reply_to, "seq": st.seq, "text": text }),
        );
        st.last_flush = Instant::now();
    }

    fn take(&self, st: &mut State, pieces: Vec<Piece>) {
        for p in pieces {
            match p {
                Piece::Text(t) => st.buffer.push_str(&t),
                Piece::Withheld => {
                    self.flush(st);
                    st.seq += 1;
                    self.publish(
                        events::AI_ANSWER_DELTA,
                        json!({ "in_reply_to": self.in_reply_to, "seq": st.seq, "withheld": "code" }),
                    );
                }
            }
        }
    }

    /// Text from the model, as it arrives.
    pub fn push(&self, delta: &str) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let pieces = st.prose.push(delta);
        self.take(&mut st, pieces);
        if st.buffer.contains('\n')
            || st.buffer.len() >= FLUSH_BYTES
            || st.last_flush.elapsed() >= FLUSH_EVERY
        {
            self.flush(&mut st);
        }
    }

    /// The model finished; the answer is about to be checked and stored.
    pub fn finish(&self) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let pieces = st.prose.finish();
        self.take(&mut st, pieces);
        self.flush(&mut st);
        self.publish(
            events::AI_ANSWER_FINISHED,
            json!({ "in_reply_to": self.in_reply_to }),
        );
    }

    /// The model failed; the draft is to be discarded.
    pub fn fail(&self) {
        self.publish(
            events::AI_ANSWER_FAILED,
            json!({ "in_reply_to": self.in_reply_to }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<DomainEvent>>);

    impl EventPublisher for Recorder {
        fn publish(&self, e: &DomainEvent) {
            self.0.lock().unwrap().push(e.clone());
        }
    }

    #[test]
    fn publishes_prose_and_withholds_code() {
        let rec = Arc::new(Recorder::default());
        let s = AnswerStream::start(rec.clone(), &"c1".into(), &"m1".into(), 3);
        for d in [
            "Look",
            " here:\n",
            "```rust\nlet secret",
            " = 1;\n```\n",
            "Why?",
        ] {
            s.push(d);
        }
        s.finish();
        let events = rec.0.lock().unwrap().clone();
        let kinds: Vec<&str> = events.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds.first(), Some(&events::AI_ANSWER_STARTED));
        assert_eq!(kinds.last(), Some(&events::AI_ANSWER_FINISHED));
        let text: String = events
            .iter()
            .filter_map(|e| e.payload["text"].as_str())
            .collect();
        assert_eq!(text, "Look here:\nWhy?");
        assert_eq!(
            events
                .iter()
                .filter(|e| e.payload["withheld"] == "code")
                .count(),
            1
        );
        assert!(!format!("{events:?}").contains("secret"));
        let seqs: Vec<u64> = events
            .iter()
            .filter_map(|e| e.payload["seq"].as_u64())
            .collect();
        assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "{seqs:?}");
        assert!(
            events
                .iter()
                .all(|e| e.conversation_id == Some("c1".into()))
        );
    }
}
