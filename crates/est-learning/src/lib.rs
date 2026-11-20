//! Interactive Learning Engine (architecture v0.3, §18).
//!
//! Learning content is data in `learning-artifacts/`, embedded into the
//! binary at build time:
//!
//! ```text
//! learning-artifacts/
//! ├── catalog.toml                    programs shown as tiles
//! ├── programs/<id>/program.toml      lessons of one program
//! ├── programs/<id>/lessons/*.{ru,en,uz}.md
//! ├── programs/<id>/quizzes/*.toml
//! ├── programs/<id>/practice/<name>/  project template + exercise.toml
//! └── artifacts/<id>/                 standalone HTML (manifest.json, index.html)
//! ```
//!
//! Every text is localized (`ru`, `en`, `uz`) with fallback to English, then
//! Russian. Quiz answers never leave the server before an attempt.

use std::collections::BTreeMap;

use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};

static CONTENT: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../learning-artifacts");

pub const LANGS: [&str; 3] = ["ru", "en", "uz"];

/// Localized text: language → string.
pub type Text = BTreeMap<String, String>;

pub fn pick(t: &Text, lang: &str) -> String {
    t.get(lang)
        .or_else(|| t.get("en"))
        .or_else(|| t.get("ru"))
        .or_else(|| t.values().next())
        .cloned()
        .unwrap_or_default()
}

#[derive(Debug)]
pub enum LearningError {
    NotFound(String),
    Invalid(String),
}

impl std::error::Error for LearningError {}

impl std::fmt::Display for LearningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LearningError::NotFound(w) => write!(f, "{w} not found"),
            LearningError::Invalid(w) => write!(f, "{w}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, LearningError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramStatus {
    Available,
    ComingSoon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramEntry {
    pub id: String,
    pub status: ProgramStatus,
    /// Icon key understood by the frontend.
    pub icon: String,
    #[serde(default)]
    pub disciplines: Vec<String>,
    pub level: String,
    pub minutes: u32,
    pub title: Text,
    pub summary: Text,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogFile {
    program: Vec<ProgramEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LessonKind {
    Reading,
    Quiz,
    Artifact,
    Practice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LessonDef {
    pub id: String,
    pub kind: LessonKind,
    pub title: Text,
    pub minutes: u32,
    /// Reading: file stem under `lessons/` (`01-run-tests` → `01-run-tests.ru.md`).
    #[serde(default)]
    pub content: Option<String>,
    /// Quiz: file under `quizzes/`.
    #[serde(default)]
    pub quiz: Option<String>,
    /// Artifact: id under `artifacts/`.
    #[serde(default)]
    pub artifact: Option<String>,
    /// Practice: directory under `practice/`.
    #[serde(default)]
    pub practice: Option<String>,
    /// Short intro shown above artifacts and practice.
    #[serde(default)]
    pub intro: Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub id: String,
    pub version: String,
    pub title: Text,
    pub summary: Text,
    #[serde(default)]
    pub disciplines: Vec<String>,
    #[serde(rename = "lesson")]
    pub lessons: Vec<LessonDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub id: String,
    pub text: Text,
    /// Optional code or output shown with the question.
    #[serde(default)]
    pub code: Option<String>,
    pub options: Vec<Text>,
    pub answer: usize,
    pub explanation: Text,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quiz {
    /// Share of correct answers needed to complete the lesson, 0–100.
    #[serde(default = "default_pass")]
    pub pass_percent: u32,
    #[serde(rename = "question")]
    pub questions: Vec<Question>,
}

fn default_pass() -> u32 {
    75
}

/// A question as sent to the learner: no answer.
#[derive(Debug, Clone, Serialize)]
pub struct QuestionView {
    pub id: String,
    pub text: String,
    pub code: Option<String>,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuestionResult {
    pub id: String,
    pub chosen: Option<usize>,
    pub correct: bool,
    pub answer: usize,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuizResult {
    pub correct: u32,
    pub total: u32,
    /// 0–100.
    pub score: u32,
    pub passed: bool,
    pub results: Vec<QuestionResult>,
}

/// Artifact manifest (v0.3 §18.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub id: String,
    pub version: String,
    pub title: Text,
    pub format: String,
    #[serde(default)]
    pub disciplines: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub learning_objectives: Vec<Text>,
    pub entrypoint: String,
}

/// Files of a practice template, path → contents.
#[derive(Debug, Clone)]
pub struct PracticeTemplate {
    pub files: Vec<(String, Vec<u8>)>,
    /// `exercise.toml` with `{{repository}}` / `{{revision}}` placeholders.
    pub manifest: String,
}

fn read_str(path: &str) -> Result<&'static str> {
    CONTENT
        .get_file(path)
        .and_then(|f| f.contents_utf8())
        .ok_or_else(|| LearningError::NotFound(path.to_owned()))
}

fn parse<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T> {
    toml::from_str(read_str(path)?).map_err(|e| LearningError::Invalid(format!("{path}: {e}")))
}

fn safe_segment(s: &str) -> Result<&str> {
    let ok = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !s.starts_with('-');
    if ok {
        Ok(s)
    } else {
        Err(LearningError::NotFound(s.to_owned()))
    }
}

pub fn catalog() -> Result<Vec<ProgramEntry>> {
    Ok(parse::<CatalogFile>("catalog.toml")?.program)
}

pub fn program(id: &str) -> Result<Program> {
    let id = safe_segment(id)?;
    let entry = catalog()?.into_iter().find(|p| p.id == id);
    match entry {
        Some(p) if p.status == ProgramStatus::Available => {
            parse(&format!("programs/{id}/program.toml"))
        }
        Some(_) => Err(LearningError::Invalid(format!(
            "program `{id}` is not available yet"
        ))),
        None => Err(LearningError::NotFound(format!("program `{id}`"))),
    }
}

pub fn lesson<'a>(program: &'a Program, id: &str) -> Result<&'a LessonDef> {
    program
        .lessons
        .iter()
        .find(|l| l.id == id)
        .ok_or_else(|| LearningError::NotFound(format!("lesson `{id}`")))
}

/// Markdown of a reading lesson in `lang` (falling back to en, then ru).
pub fn reading(program: &Program, lesson: &LessonDef, lang: &str) -> Result<String> {
    let stem = safe_segment(lesson.content.as_deref().unwrap_or_default())?;
    for l in [lang, "en", "ru"] {
        if let Ok(s) = read_str(&format!("programs/{}/lessons/{stem}.{l}.md", program.id)) {
            return Ok(s.to_owned());
        }
    }
    Err(LearningError::NotFound(format!("lesson text `{stem}`")))
}

pub fn quiz(program: &Program, lesson: &LessonDef) -> Result<Quiz> {
    let name = safe_segment(lesson.quiz.as_deref().unwrap_or_default())?;
    let q: Quiz = parse(&format!("programs/{}/quizzes/{name}.toml", program.id))?;
    for question in &q.questions {
        if question.answer >= question.options.len() {
            return Err(LearningError::Invalid(format!(
                "quiz {name}: answer out of range in {}",
                question.id
            )));
        }
    }
    Ok(q)
}

pub fn quiz_view(q: &Quiz, lang: &str) -> Vec<QuestionView> {
    q.questions
        .iter()
        .map(|x| QuestionView {
            id: x.id.clone(),
            text: pick(&x.text, lang),
            code: x.code.clone(),
            options: x.options.iter().map(|o| pick(o, lang)).collect(),
        })
        .collect()
}

pub fn grade(q: &Quiz, answers: &BTreeMap<String, usize>, lang: &str) -> QuizResult {
    let results: Vec<QuestionResult> = q
        .questions
        .iter()
        .map(|x| {
            let chosen = answers.get(&x.id).copied();
            QuestionResult {
                id: x.id.clone(),
                chosen,
                correct: chosen == Some(x.answer),
                answer: x.answer,
                explanation: pick(&x.explanation, lang),
            }
        })
        .collect();
    let total = results.len() as u32;
    let correct = results.iter().filter(|r| r.correct).count() as u32;
    let score = (correct * 100).checked_div(total).unwrap_or(0);
    QuizResult {
        correct,
        total,
        score,
        passed: score >= q.pass_percent,
        results,
    }
}

pub fn artifact_manifest(id: &str) -> Result<ArtifactManifest> {
    let id = safe_segment(id)?;
    serde_json::from_str(read_str(&format!("artifacts/{id}/manifest.json"))?)
        .map_err(|e| LearningError::Invalid(format!("artifact {id}: {e}")))
}

/// A file of an artifact. Only files inside the artifact's directory.
pub fn artifact_file(id: &str, path: &str) -> Result<(&'static [u8], &'static str)> {
    let id = safe_segment(id)?;
    if path
        .split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.'))
    {
        return Err(LearningError::NotFound(path.to_owned()));
    }
    let file = CONTENT
        .get_file(format!("artifacts/{id}/{path}"))
        .ok_or_else(|| LearningError::NotFound(path.to_owned()))?;
    let mime = match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    };
    Ok((file.contents(), mime))
}

pub fn practice_template(program: &Program, lesson: &LessonDef) -> Result<PracticeTemplate> {
    let name = safe_segment(lesson.practice.as_deref().unwrap_or_default())?;
    let root = format!("programs/{}/practice/{name}", program.id);
    let dir = CONTENT
        .get_dir(&root)
        .ok_or_else(|| LearningError::NotFound(format!("practice `{name}`")))?;
    let mut files = Vec::new();
    let mut manifest = None;
    collect(dir, &root, &mut files, &mut manifest);
    Ok(PracticeTemplate {
        files,
        manifest: manifest.ok_or_else(|| {
            LearningError::Invalid(format!("practice `{name}` has no exercise.toml"))
        })?,
    })
}

fn collect(
    dir: &Dir<'static>,
    root: &str,
    files: &mut Vec<(String, Vec<u8>)>,
    manifest: &mut Option<String>,
) {
    for f in dir.files() {
        let rel = f
            .path()
            .to_string_lossy()
            .trim_start_matches(root)
            .trim_start_matches('/')
            .to_owned();
        if rel == "exercise.toml" {
            *manifest = f.contents_utf8().map(str::to_owned);
        } else {
            // Templates store `Cargo.toml` as `Cargo.toml.tmpl` so the
            // workspace does not mistake them for member crates.
            let rel = rel.strip_suffix(".tmpl").map(str::to_owned).unwrap_or(rel);
            files.push((rel, f.contents().to_vec()));
        }
    }
    for d in dir.dirs() {
        collect(d, root, files, manifest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_content_is_consistent() {
        let cat = catalog().unwrap();
        assert!(cat.iter().any(|p| p.status == ProgramStatus::Available));
        for entry in &cat {
            for l in LANGS {
                assert!(!pick(&entry.title, l).is_empty(), "{} title {l}", entry.id);
            }
            if entry.status != ProgramStatus::Available {
                assert!(program(&entry.id).is_err());
                continue;
            }
            let p = program(&entry.id).unwrap();
            assert_eq!(p.id, entry.id);
            for lesson in &p.lessons {
                match lesson.kind {
                    LessonKind::Reading => {
                        for l in ["ru", "en"] {
                            assert!(
                                reading(&p, lesson, l).unwrap().len() > 200,
                                "{} {l}",
                                lesson.id
                            );
                        }
                    }
                    LessonKind::Quiz => {
                        let q = quiz(&p, lesson).unwrap();
                        assert!(!q.questions.is_empty());
                        for question in &q.questions {
                            for l in ["ru", "en"] {
                                assert!(
                                    question.text.contains_key(l)
                                        && question.explanation.contains_key(l),
                                    "{} {l}",
                                    question.id
                                );
                            }
                        }
                    }
                    LessonKind::Artifact => {
                        let m = artifact_manifest(lesson.artifact.as_deref().unwrap()).unwrap();
                        artifact_file(&m.id, &m.entrypoint).unwrap();
                    }
                    LessonKind::Practice => {
                        let t = practice_template(&p, lesson).unwrap();
                        assert!(
                            t.manifest.contains("{{repository}}")
                                && t.manifest.contains("{{revision}}")
                        );
                        assert!(t.files.iter().any(|(p, _)| p == "Cargo.toml"));
                    }
                }
            }
        }
    }

    #[test]
    fn grading_hides_nothing_after_the_attempt() {
        let p = program("testing").unwrap();
        let lesson = p
            .lessons
            .iter()
            .find(|l| l.kind == LessonKind::Quiz)
            .unwrap();
        let q = quiz(&p, lesson).unwrap();
        let all_right: BTreeMap<_, _> = q
            .questions
            .iter()
            .map(|x| (x.id.clone(), x.answer))
            .collect();
        let r = grade(&q, &all_right, "en");
        assert_eq!((r.score, r.passed), (100, true));
        let r = grade(&q, &BTreeMap::new(), "en");
        assert_eq!((r.score, r.passed), (0, false));
        assert!(r.results.iter().all(|x| !x.explanation.is_empty()));
        // The view sent before an attempt has no answers.
        let view = serde_json::to_string(&quiz_view(&q, "ru")).unwrap();
        assert!(!view.contains("answer") && !view.contains("explanation"));
    }

    #[test]
    fn paths_cannot_escape() {
        assert!(artifact_file("boundary-values", "../../catalog.toml").is_err());
        assert!(artifact_file("..", "catalog.toml").is_err());
        assert!(artifact_file("boundary-values", ".hidden").is_err());
        assert!(program("../x").is_err());
    }

    #[test]
    fn fallback_order() {
        let t: Text = [
            ("ru".to_owned(), "привет".to_owned()),
            ("en".to_owned(), "hello".to_owned()),
        ]
        .into();
        assert_eq!(pick(&t, "uz"), "hello");
        assert_eq!(pick(&t, "ru"), "привет");
    }
}
