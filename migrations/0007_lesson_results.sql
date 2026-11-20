-- Results of quiz attempts and artifacts, kept as Skill Matrix evidence with
-- their history (lesson_progress keeps only the best score). Which skills a
-- result is evidence for is read from the program at assessment time.

CREATE TABLE lesson_results (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     TEXT NOT NULL REFERENCES users(id),
    program_id  TEXT NOT NULL,
    lesson_id   TEXT NOT NULL,
    source      TEXT NOT NULL,           -- quiz | artifact
    score       INTEGER NOT NULL CHECK (score BETWEEN 0 AND 100),
    observed_at TEXT NOT NULL
);
CREATE INDEX lesson_results_user_idx ON lesson_results(user_id, observed_at);
