-- Searches for Issues and problems planned by a model and run by EST.

CREATE TABLE discoveries (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(id),
    created_at  TEXT NOT NULL,
    model       TEXT NOT NULL,          -- provider/model (no secrets)
    data        TEXT NOT NULL           -- JSON: goal, summary, candidates, searches
);
CREATE INDEX discoveries_user_idx ON discoveries(user_id, created_at);
