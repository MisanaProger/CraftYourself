-- Learning recommendations written by a model from the learner's activity.

CREATE TABLE recommendations (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(id),
    created_at  TEXT NOT NULL,
    model       TEXT NOT NULL,          -- provider/model (no secrets)
    lang        TEXT NOT NULL,
    summary     TEXT NOT NULL,
    items       TEXT NOT NULL,          -- JSON array, validated before it is stored
    activity    TEXT NOT NULL           -- JSON: exactly what the model was given
);
CREATE INDEX recommendations_user_idx ON recommendations(user_id, created_at);
