-- Session Summaries: what happened in a finished session, built in code from
-- observed data (counts, ids and the task title; no messages, no code).
-- The Knowledge Profile is computed from these rows on request.

CREATE TABLE session_summaries (
    session_id  TEXT PRIMARY KEY REFERENCES learning_sessions(id) ON DELETE CASCADE,
    user_id     TEXT NOT NULL REFERENCES users(id),
    task_id     TEXT NOT NULL,
    finished_at TEXT NOT NULL,
    summary     TEXT NOT NULL            -- JSON SessionSummary
);
CREATE INDEX session_summaries_user_idx ON session_summaries(user_id, finished_at);
