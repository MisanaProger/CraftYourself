//! SQLite implementation of the repository ports (architecture v0.2, §12).
//!
//! Queries are checked at runtime, so the crate builds without a database.
//! A PostgreSQL implementation can provide the same ports later.

use std::path::Path;
use std::str::FromStr;

use async_trait::async_trait;
use est_application::ports::{
    EventLog, ProjectRepository, SessionRepository, StoredEvent, TaskRepository, User,
    UserRepository,
};
use est_application::{AppError, AppResult};
use est_core::{DomainEvent, ProjectId, SessionId, TaskId, Timestamp, UserId};
use est_domain::{
    LanguageDetection, LearningSession, NewProject, NewTask, Project, SessionStatus, Task,
    TaskSource, TaskStatus,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

fn db(e: sqlx::Error) -> AppError {
    match &e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            AppError::Conflict(format!("already exists: {}", d.message()))
        }
        _ => AppError::Infrastructure(format!("database: {e}")),
    }
}

fn corrupt(what: &str, e: impl std::fmt::Display) -> AppError {
    AppError::Infrastructure(format!("stored {what} is invalid: {e}"))
}

/// `TaskType::BugFix` → `"bug-fix"`.
fn enum_str<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        other => panic!("not a unit enum: {other:?}"),
    }
}

fn parse_enum<T: DeserializeOwned>(what: &str, s: &str) -> AppResult<T> {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).map_err(|e| corrupt(what, e))
}

fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("domain values serialize")
}

fn from_json<T: DeserializeOwned>(what: &str, s: &str) -> AppResult<T> {
    serde_json::from_str(s).map_err(|e| corrupt(what, e))
}

impl SqliteStore {
    /// Opens (creating if needed) a database file and applies migrations.
    pub async fn open(path: &Path) -> AppResult<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5));
        Self::connect(opts, 8).await
    }

    /// In-memory database, for tests.
    pub async fn in_memory() -> AppResult<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(db)?
            .foreign_keys(true);
        // One connection: each in-memory connection is a separate database.
        Self::connect(opts, 1).await
    }

    async fn connect(opts: SqliteConnectOptions, max: u32) -> AppResult<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(max)
            .connect_with(opts)
            .await
            .map_err(db)?;
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .map_err(|e| AppError::Infrastructure(format!("migration failed: {e}")))?;
        Ok(Self { pool })
    }

    async fn languages(&self, project: &str) -> AppResult<Vec<LanguageDetection>> {
        let rows = sqlx::query(
            "SELECT language, files, build_systems FROM project_languages \
             WHERE project_id = ? ORDER BY files DESC, language",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        rows.iter()
            .map(|r| {
                Ok(LanguageDetection {
                    language: r.get("language"),
                    files: r.get::<i64, _>("files") as u32,
                    build_systems: from_json("build systems", r.get("build_systems"))?,
                })
            })
            .collect()
    }

    async fn project_from_row(&self, r: &SqliteRow) -> AppResult<Project> {
        let id: String = r.get("id");
        Ok(Project {
            languages: self.languages(&id).await?,
            id: ProjectId::from(id),
            name: r.get("name"),
            repository_url: r.get("repository_url"),
            local_path: r.get::<String, _>("local_path").into(),
            primary_language: r.get("primary_language"),
            license: r.get("license"),
            created_at: r.get("created_at"),
        })
    }

    async fn projects_where(&self, clause: &str, arg: Option<&str>) -> AppResult<Vec<Project>> {
        let sql = format!("SELECT * FROM projects {clause} ORDER BY created_at DESC");
        let mut q = sqlx::query(&sql);
        if let Some(a) = arg {
            q = q.bind(a);
        }
        let rows = q.fetch_all(&self.pool).await.map_err(db)?;
        let mut out = Vec::with_capacity(rows.len());
        for r in &rows {
            out.push(self.project_from_row(r).await?);
        }
        Ok(out)
    }
}

fn task_from_row(r: &SqliteRow) -> AppResult<Task> {
    Ok(Task {
        id: TaskId::from(r.get::<String, _>("id")),
        project_id: ProjectId::from(r.get::<String, _>("project_id")),
        title: r.get("title"),
        description: r.get("description"),
        task_type: parse_enum("task type", r.get("task_type"))?,
        difficulty: parse_enum("difficulty", r.get("difficulty"))?,
        source: from_json("task source", r.get("source"))?,
        status: parse_enum("task status", r.get("status"))?,
        learning_goals: from_json("learning goals", r.get("learning_goals"))?,
        skills: from_json("skills", r.get("skills"))?,
        base_revision: r.get("base_revision"),
        test_command: r.get("test_command"),
        mentor: from_json("mentor settings", r.get("mentor"))?,
        docs: from_json("docs", r.get("docs"))?,
        created_at: r.get("created_at"),
    })
}

fn session_from_row(r: &SqliteRow) -> AppResult<LearningSession> {
    let status: String = r.get("status");
    Ok(LearningSession {
        id: SessionId::from(r.get::<String, _>("id")),
        user_id: UserId::from(r.get::<String, _>("user_id")),
        project_id: ProjectId::from(r.get::<String, _>("project_id")),
        task_id: TaskId::from(r.get::<String, _>("task_id")),
        workspace_path: r.get::<String, _>("workspace_path").into(),
        branch: r.get("branch"),
        base_revision: r.get("base_revision"),
        status: SessionStatus::parse(&status).ok_or_else(|| corrupt("session status", &status))?,
        started_at: r.get("started_at"),
        finished_at: r.get("finished_at"),
    })
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[async_trait]
impl UserRepository for SqliteStore {
    async fn local_user(&self) -> AppResult<User> {
        if let Some(r) =
            sqlx::query("SELECT id, display_name FROM users WHERE is_local = 1 LIMIT 1")
                .fetch_optional(&self.pool)
                .await
                .map_err(db)?
        {
            return Ok(User {
                id: UserId::from(r.get::<String, _>("id")),
                display_name: r.get("display_name"),
            });
        }
        let user = User {
            id: UserId::generate(),
            display_name: "Local learner".into(),
        };
        sqlx::query(
            "INSERT INTO users (id, display_name, is_local, created_at) VALUES (?, ?, 1, ?)",
        )
        .bind(user.id.as_str())
        .bind(&user.display_name)
        .bind(est_core::now())
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(user)
    }
}

#[async_trait]
impl ProjectRepository for SqliteStore {
    async fn create(&self, p: NewProject) -> AppResult<Project> {
        let id = ProjectId::generate();
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query(
            "INSERT INTO projects (id, name, repository_url, local_path, primary_language, license, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.as_str())
        .bind(&p.name)
        .bind(&p.repository_url)
        .bind(path_str(&p.local_path))
        .bind(p.primary_language())
        .bind(&p.license)
        .bind(est_core::now())
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        for l in &p.languages {
            sqlx::query(
                "INSERT INTO project_languages (project_id, language, files, build_systems) VALUES (?, ?, ?, ?)",
            )
            .bind(id.as_str())
            .bind(&l.language)
            .bind(i64::from(l.files))
            .bind(to_json(&l.build_systems))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        }
        tx.commit().await.map_err(db)?;
        ProjectRepository::get(self, &id)
            .await?
            .ok_or_else(|| AppError::Infrastructure("project vanished after insert".into()))
    }

    async fn get(&self, id: &ProjectId) -> AppResult<Option<Project>> {
        Ok(self
            .projects_where("WHERE id = ?", Some(id.as_str()))
            .await?
            .pop())
    }

    async fn find_by_path(&self, path: &Path) -> AppResult<Option<Project>> {
        Ok(self
            .projects_where("WHERE local_path = ?", Some(&path_str(path)))
            .await?
            .pop())
    }

    async fn find_by_url(&self, url: &str) -> AppResult<Option<Project>> {
        Ok(self
            .projects_where("WHERE repository_url = ?", Some(url))
            .await?
            .pop())
    }

    async fn list(&self) -> AppResult<Vec<Project>> {
        self.projects_where("", None).await
    }
}

#[async_trait]
impl TaskRepository for SqliteStore {
    async fn create(&self, t: NewTask) -> AppResult<Task> {
        let id = TaskId::generate();
        let (exercise_id, exercise_version) = match &t.source {
            TaskSource::Exercise { id, version } => (Some(id.clone()), Some(version.clone())),
            _ => (None, None),
        };
        sqlx::query(
            "INSERT INTO tasks (id, project_id, title, description, task_type, difficulty, status, source, \
             exercise_id, exercise_version, learning_goals, skills, base_revision, test_command, mentor, docs, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.as_str())
        .bind(t.project_id.as_str())
        .bind(t.title.trim())
        .bind(&t.description)
        .bind(enum_str(&t.task_type))
        .bind(enum_str(&t.difficulty))
        .bind(enum_str(&TaskStatus::Open))
        .bind(to_json(&t.source))
        .bind(exercise_id)
        .bind(exercise_version)
        .bind(to_json(&t.learning_goals))
        .bind(to_json(&t.skills))
        .bind(&t.base_revision)
        .bind(&t.test_command)
        .bind(to_json(&t.mentor.clone().unwrap_or_default()))
        .bind(to_json(&t.docs))
        .bind(est_core::now())
        .execute(&self.pool)
        .await
        .map_err(db)?;
        TaskRepository::get(self, &id)
            .await?
            .ok_or_else(|| AppError::Infrastructure("task vanished after insert".into()))
    }

    async fn get(&self, id: &TaskId) -> AppResult<Option<Task>> {
        sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .as_ref()
            .map(task_from_row)
            .transpose()
    }

    async fn list(&self, project: Option<&ProjectId>) -> AppResult<Vec<Task>> {
        let rows = match project {
            Some(p) => {
                sqlx::query("SELECT * FROM tasks WHERE project_id = ? ORDER BY created_at DESC")
                    .bind(p.as_str())
                    .fetch_all(&self.pool)
                    .await
            }
            None => {
                sqlx::query("SELECT * FROM tasks ORDER BY created_at DESC")
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(db)?;
        rows.iter().map(task_from_row).collect()
    }

    async fn set_status(&self, id: &TaskId, status: TaskStatus) -> AppResult<()> {
        let res = sqlx::query("UPDATE tasks SET status = ? WHERE id = ?")
            .bind(enum_str(&status))
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(db)?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("task".into()));
        }
        Ok(())
    }

    async fn find_exercise(
        &self,
        project: &ProjectId,
        exercise_id: &str,
        version: &str,
    ) -> AppResult<Option<Task>> {
        sqlx::query(
            "SELECT * FROM tasks WHERE project_id = ? AND exercise_id = ? AND exercise_version = ?",
        )
        .bind(project.as_str())
        .bind(exercise_id)
        .bind(version)
        .fetch_optional(&self.pool)
        .await
        .map_err(db)?
        .as_ref()
        .map(task_from_row)
        .transpose()
    }
}

#[async_trait]
impl SessionRepository for SqliteStore {
    async fn create(&self, s: &LearningSession) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO learning_sessions (id, user_id, project_id, task_id, workspace_path, branch, \
             base_revision, status, started_at, finished_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(s.id.as_str())
        .bind(s.user_id.as_str())
        .bind(s.project_id.as_str())
        .bind(s.task_id.as_str())
        .bind(path_str(&s.workspace_path))
        .bind(&s.branch)
        .bind(&s.base_revision)
        .bind(s.status.as_str())
        .bind(s.started_at)
        .bind(s.finished_at)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    async fn update(&self, s: &LearningSession) -> AppResult<()> {
        let res =
            sqlx::query("UPDATE learning_sessions SET status = ?, finished_at = ? WHERE id = ?")
                .bind(s.status.as_str())
                .bind(s.finished_at)
                .bind(s.id.as_str())
                .execute(&self.pool)
                .await
                .map_err(db)?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("session".into()));
        }
        Ok(())
    }

    async fn get(&self, id: &SessionId) -> AppResult<Option<LearningSession>> {
        sqlx::query("SELECT * FROM learning_sessions WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .as_ref()
            .map(session_from_row)
            .transpose()
    }

    async fn list(&self, user: &UserId) -> AppResult<Vec<LearningSession>> {
        sqlx::query("SELECT * FROM learning_sessions WHERE user_id = ? ORDER BY started_at DESC")
            .bind(user.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(db)?
            .iter()
            .map(session_from_row)
            .collect()
    }

    async fn active_for_task(&self, task: &TaskId) -> AppResult<Option<LearningSession>> {
        sqlx::query("SELECT * FROM learning_sessions WHERE task_id = ? AND status = 'active'")
            .bind(task.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .as_ref()
            .map(session_from_row)
            .transpose()
    }

    async fn find_by_workspace(&self, path: &Path) -> AppResult<Option<LearningSession>> {
        // Exact match, or `path` lies inside the workspace directory.
        sqlx::query(
            "SELECT * FROM learning_sessions \
             WHERE workspace_path = ?1 OR substr(?1, 1, length(workspace_path) + 1) = workspace_path || '/' \
             ORDER BY status = 'active' DESC, started_at DESC LIMIT 1",
        )
        .bind(path_str(path))
        .fetch_optional(&self.pool)
        .await
        .map_err(db)?
        .as_ref()
        .map(session_from_row)
        .transpose()
    }
}

#[async_trait]
impl EventLog for SqliteStore {
    async fn append(&self, e: &DomainEvent) -> AppResult<i64> {
        Ok(sqlx::query(
            "INSERT INTO session_events (kind, session_id, project_id, task_id, payload, at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&e.kind)
        .bind(e.session_id.as_ref().map(SessionId::as_str))
        .bind(e.project_id.as_ref().map(ProjectId::as_str))
        .bind(e.task_id.as_ref().map(TaskId::as_str))
        .bind(e.payload.to_string())
        .bind(e.at)
        .execute(&self.pool)
        .await
        .map_err(db)?
        .last_insert_rowid())
    }

    async fn for_session(
        &self,
        session: &SessionId,
        kind: Option<&str>,
    ) -> AppResult<Vec<StoredEvent>> {
        let rows = sqlx::query(
            "SELECT * FROM session_events WHERE session_id = ?1 AND (?2 IS NULL OR kind = ?2) ORDER BY id",
        )
        .bind(session.as_str())
        .bind(kind)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?;
        rows.iter()
            .map(|r| {
                Ok(StoredEvent {
                    id: r.get("id"),
                    event: DomainEvent {
                        kind: r.get("kind"),
                        session_id: r
                            .get::<Option<String>, _>("session_id")
                            .map(SessionId::from),
                        project_id: r
                            .get::<Option<String>, _>("project_id")
                            .map(ProjectId::from),
                        task_id: r.get::<Option<String>, _>("task_id").map(TaskId::from),
                        payload: from_json("event payload", r.get("payload"))?,
                        at: r.get::<Timestamp, _>("at"),
                    },
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use est_domain::{Difficulty, HelpLevel, MentorSettings, TaskType};
    use serde_json::json;

    async fn seeded() -> (SqliteStore, Project, Task, User) {
        let s = SqliteStore::in_memory().await.unwrap();
        let user = s.local_user().await.unwrap();
        let p = ProjectRepository::create(
            &s,
            NewProject {
                name: "ranges".into(),
                repository_url: Some("https://example.invalid/ranges".into()),
                local_path: "/tmp/ranges".into(),
                languages: vec![
                    LanguageDetection {
                        language: "rust".into(),
                        files: 3,
                        build_systems: vec!["cargo".into()],
                    },
                    LanguageDetection {
                        language: "python".into(),
                        files: 1,
                        build_systems: vec![],
                    },
                ],
                license: Some("MIT".into()),
            },
        )
        .await
        .unwrap();
        let t = TaskRepository::create(
            &s,
            NewTask {
                project_id: p.id.clone(),
                title: "Fix range".into(),
                description: String::new(),
                task_type: TaskType::BugFix,
                difficulty: Difficulty::Beginner,
                source: TaskSource::Exercise {
                    id: "fix-range".into(),
                    version: "1.0.0".into(),
                },
                learning_goals: vec!["goal".into()],
                skills: vec![est_domain::Skill::Debugging],
                base_revision: None,
                test_command: Some("cargo test".into()),
                mentor: Some(MentorSettings {
                    max_help_level: HelpLevel::ConceptualHints,
                    ..Default::default()
                }),
                docs: vec![],
            },
        )
        .await
        .unwrap();
        (s, p, t, user)
    }

    fn session(user: &User, p: &Project, t: &Task, ws: &str) -> LearningSession {
        LearningSession {
            id: SessionId::generate(),
            user_id: user.id.clone(),
            project_id: p.id.clone(),
            task_id: t.id.clone(),
            workspace_path: ws.into(),
            branch: "est/x".into(),
            base_revision: "a".repeat(40),
            status: SessionStatus::Active,
            started_at: est_core::now(),
            finished_at: None,
        }
    }

    #[tokio::test]
    async fn projects_roundtrip() {
        let (s, p, _, user) = seeded().await;
        assert_eq!(p.primary_language.as_deref(), Some("rust"));
        assert_eq!(p.languages.len(), 2);
        assert_eq!(
            s.find_by_path(Path::new("/tmp/ranges"))
                .await
                .unwrap()
                .unwrap()
                .id,
            p.id
        );
        assert!(
            s.find_by_url("https://example.invalid/ranges")
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(s.local_user().await.unwrap().id, user.id);
    }

    #[tokio::test]
    async fn tasks_roundtrip() {
        let (s, p, t, _) = seeded().await;
        assert_eq!(t.status, TaskStatus::Open);
        assert_eq!(t.mentor.max_help_level, HelpLevel::ConceptualHints);
        assert!(
            s.find_exercise(&p.id, "fix-range", "1.0.0")
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            s.find_exercise(&p.id, "fix-range", "2.0.0")
                .await
                .unwrap()
                .is_none()
        );
        s.set_status(&t.id, TaskStatus::InProgress).await.unwrap();
        assert_eq!(
            TaskRepository::get(&s, &t.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::InProgress
        );
        assert_eq!(
            TaskRepository::list(&s, Some(&p.id)).await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn sessions_and_events() {
        let (s, p, t, user) = seeded().await;
        let mut a = session(&user, &p, &t, "/ws/p/t");
        SessionRepository::create(&s, &a).await.unwrap();
        // Only one active session per task.
        assert!(
            SessionRepository::create(&s, &session(&user, &p, &t, "/ws/p/t2"))
                .await
                .is_err()
        );

        assert_eq!(s.active_for_task(&t.id).await.unwrap().unwrap().id, a.id);
        assert_eq!(
            s.find_by_workspace(Path::new("/ws/p/t/src/lib.rs"))
                .await
                .unwrap()
                .unwrap()
                .id,
            a.id
        );
        assert!(
            s.find_by_workspace(Path::new("/ws/p/t-other"))
                .await
                .unwrap()
                .is_none()
        );

        a.finish(est_domain::SessionOutcome::Completed, est_core::now())
            .unwrap();
        s.update(&a).await.unwrap();
        assert!(s.active_for_task(&t.id).await.unwrap().is_none());

        s.append(&DomainEvent::new("mentor.request", json!({"level": 2})).session(&a.id))
            .await
            .unwrap();
        s.append(&DomainEvent::new("session.finished", json!({})).session(&a.id))
            .await
            .unwrap();
        s.append(&DomainEvent::new("project.added", json!({})).project(&p.id))
            .await
            .unwrap();
        assert_eq!(s.for_session(&a.id, None).await.unwrap().len(), 2);
        let m = s.for_session(&a.id, Some("mentor.request")).await.unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].event.payload["level"], 2);
    }
}
