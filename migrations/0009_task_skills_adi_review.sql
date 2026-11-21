-- Skills a task exercises (`discipline/skill`), for Skill Matrix evidence
-- at skill level; the column `skills` keeps the task's discipline ids.
ALTER TABLE tasks ADD COLUMN skill_refs TEXT NOT NULL DEFAULT '[]';

-- A model-written explanation of an Issue's ADI (JSON: text, model,
-- created_at). It never changes the score; a new assessment clears it.
ALTER TABLE adi_assessments ADD COLUMN review TEXT;
