-- Engineering Skills Trainer: initial schema (Local Edition, SQLite).
-- Portable SQL only, so the same schema can back PostgreSQL later.

CREATE TABLE users (
    id           TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    -- Local Edition works without registration: one implicit local user.
    is_local     INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL
);

CREATE TABLE projects (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    repository_url   TEXT UNIQUE,
    local_path       TEXT NOT NULL UNIQUE,
    primary_language TEXT,
    license          TEXT,
    created_at       TEXT NOT NULL
);

CREATE TABLE project_languages (
    project_id    TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    language      TEXT NOT NULL,
    files         INTEGER NOT NULL,
    build_systems TEXT NOT NULL, -- JSON array
    PRIMARY KEY (project_id, language)
);

CREATE TABLE tasks (
    id               TEXT PRIMARY KEY,
    project_id       TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title            TEXT NOT NULL,
    description      TEXT NOT NULL,
    task_type        TEXT NOT NULL,
    difficulty       TEXT NOT NULL,
    status           TEXT NOT NULL,
    source           TEXT NOT NULL, -- JSON TaskSource
    exercise_id      TEXT,
    exercise_version TEXT,
    learning_goals   TEXT NOT NULL, -- JSON array
    skills           TEXT NOT NULL, -- JSON array
    base_revision    TEXT,
    test_command     TEXT,
    mentor           TEXT NOT NULL, -- JSON MentorSettings
    docs             TEXT NOT NULL, -- JSON array
    created_at       TEXT NOT NULL
);
CREATE INDEX tasks_project_idx ON tasks(project_id);
CREATE UNIQUE INDEX tasks_exercise_idx ON tasks(project_id, exercise_id, exercise_version)
    WHERE exercise_id IS NOT NULL;

CREATE TABLE learning_sessions (
    id             TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES users(id),
    project_id     TEXT NOT NULL REFERENCES projects(id),
    task_id        TEXT NOT NULL REFERENCES tasks(id),
    workspace_path TEXT NOT NULL,
    branch         TEXT NOT NULL,
    base_revision  TEXT NOT NULL,
    status         TEXT NOT NULL CHECK (status IN ('active', 'completed', 'abandoned')),
    started_at     TEXT NOT NULL,
    finished_at    TEXT
);
CREATE INDEX sessions_user_idx ON learning_sessions(user_id);
-- At most one active session per task.
CREATE UNIQUE INDEX sessions_active_task_idx ON learning_sessions(task_id) WHERE status = 'active';

-- Append-only log of observable actions; assessment is built from it.
CREATE TABLE session_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT NOT NULL,
    session_id TEXT REFERENCES learning_sessions(id),
    project_id TEXT,
    task_id    TEXT,
    payload    TEXT NOT NULL,
    at         TEXT NOT NULL
);
CREATE INDEX events_session_idx ON session_events(session_id, kind);
