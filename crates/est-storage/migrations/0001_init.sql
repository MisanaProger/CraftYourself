-- Engineering Skills Trainer: initial schema.
-- Kept to portable SQL so the same schema can be ported to PostgreSQL.

CREATE TABLE users (
    id           TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    -- Local Edition works without registration: one implicit local user.
    is_local     INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL
);

CREATE TABLE projects (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    description    TEXT,
    repository_url TEXT NOT NULL UNIQUE,
    license        TEXT,
    language       TEXT,
    default_branch TEXT,
    head_revision  TEXT NOT NULL,
    local_path     TEXT NOT NULL,
    created_at     TEXT NOT NULL
);

CREATE TABLE exercises (
    id            TEXT PRIMARY KEY,
    version       TEXT NOT NULL,
    title         TEXT NOT NULL,
    category      TEXT NOT NULL,
    difficulty    TEXT NOT NULL,
    repository    TEXT NOT NULL,
    revision      TEXT NOT NULL,
    manifest_toml TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE sessions (
    id             TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES users(id),
    exercise_id    TEXT NOT NULL REFERENCES exercises(id),
    project_id     TEXT NOT NULL REFERENCES projects(id),
    workspace_path TEXT NOT NULL,
    branch         TEXT NOT NULL,
    base_revision  TEXT NOT NULL,
    status         TEXT NOT NULL CHECK (status IN ('active', 'completed', 'abandoned')),
    started_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);
CREATE INDEX sessions_user_idx ON sessions(user_id);

-- Append-only log of observable learner actions (spec 7: assessment is
-- based on observed actions). Mentor hint usage is recorded here.
CREATE TABLE events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    kind       TEXT NOT NULL,
    payload    TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX events_session_idx ON events(session_id, kind);
