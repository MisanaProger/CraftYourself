-- Acceptance criteria, sandbox runs and learning progress.

ALTER TABLE tasks ADD COLUMN acceptance_criteria TEXT NOT NULL DEFAULT '[]'; -- JSON array of {text, done}

CREATE TABLE sandbox_runs (
    id            TEXT PRIMARY KEY,
    session_id    TEXT NOT NULL REFERENCES learning_sessions(id) ON DELETE CASCADE,
    task_id       TEXT NOT NULL,
    kind          TEXT NOT NULL,           -- test | build | check | setup | custom
    command       TEXT NOT NULL,
    workdir       TEXT NOT NULL,
    image         TEXT,
    network       INTEGER NOT NULL,
    status        TEXT NOT NULL CHECK (status IN ('running', 'passed', 'failed', 'timed_out', 'error')),
    exit_code     INTEGER,
    passed        INTEGER,
    failed        INTEGER,
    ignored       INTEGER,
    duration_ms   INTEGER,
    stdout        TEXT NOT NULL DEFAULT '',
    stderr        TEXT NOT NULL DEFAULT '',
    error         TEXT,
    started_at    TEXT NOT NULL,
    finished_at   TEXT
);
CREATE INDEX sandbox_runs_session_idx ON sandbox_runs(session_id, started_at);

CREATE TABLE learning_progress (
    user_id      TEXT NOT NULL REFERENCES users(id),
    program_id   TEXT NOT NULL,
    lesson_id    TEXT NOT NULL,
    status       TEXT NOT NULL CHECK (status IN ('started', 'completed')),
    score        INTEGER,
    attempts     INTEGER NOT NULL DEFAULT 0,
    detail       TEXT NOT NULL DEFAULT '{}', -- JSON: quiz answers, artifact result
    updated_at   TEXT NOT NULL,
    PRIMARY KEY (user_id, program_id, lesson_id)
);
