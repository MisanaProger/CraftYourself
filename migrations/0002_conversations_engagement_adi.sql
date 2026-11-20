-- Architecture v0.3: chat-first conversations, Engagement Controller, ADI.

CREATE TABLE conversations (
    id         TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    task_id    TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    status     TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
-- One conversation per Issue.
CREATE UNIQUE INDEX conversations_task_idx ON conversations(task_id) WHERE task_id IS NOT NULL;

CREATE TABLE conversation_messages (
    seq             INTEGER PRIMARY KEY AUTOINCREMENT,
    id              TEXT NOT NULL UNIQUE,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content         TEXT NOT NULL,
    cards           TEXT NOT NULL, -- JSON array of cards
    level           INTEGER NOT NULL,
    meta            TEXT NOT NULL, -- JSON
    created_at      TEXT NOT NULL
);
CREATE INDEX messages_conversation_idx ON conversation_messages(conversation_id, seq);

CREATE TABLE engagement_states (
    conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
    level           INTEGER NOT NULL CHECK (level BETWEEN 0 AND 4),
    max_level       INTEGER NOT NULL CHECK (max_level BETWEEN 0 AND 4),
    updated_at      TEXT NOT NULL
);

CREATE TABLE engagement_escalations (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    from_level      INTEGER NOT NULL,
    to_level        INTEGER NOT NULL,
    required_level  INTEGER NOT NULL,
    message_id      TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('proposed', 'accepted', 'rejected')),
    created_at      TEXT NOT NULL,
    resolved_at     TEXT
);
CREATE INDEX escalations_conversation_idx ON engagement_escalations(conversation_id);

-- Final ADI per completed Issue, stored with the scoring version.
CREATE TABLE adi_assessments (
    sequence          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id           TEXT NOT NULL UNIQUE REFERENCES tasks(id) ON DELETE CASCADE,
    score             INTEGER NOT NULL CHECK (score BETWEEN 0 AND 100),
    scoring_version   TEXT NOT NULL,
    observed_features TEXT NOT NULL,
    factors           TEXT NOT NULL,
    explanation       TEXT NOT NULL,
    confidence        TEXT NOT NULL,
    created_at        TEXT NOT NULL
);

ALTER TABLE learning_sessions ADD COLUMN conversation_id TEXT;
ALTER TABLE session_events ADD COLUMN conversation_id TEXT;
CREATE INDEX events_conversation_idx ON session_events(conversation_id, kind);
