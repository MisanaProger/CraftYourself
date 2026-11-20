//! SQLite implementation of the repository ports (architecture v0.2, §12).
//!
//! Queries are checked at runtime, so the crate builds without a database.
//! A PostgreSQL implementation can provide the same ports later.

use std::path::Path;
use std::str::FromStr;

use async_trait::async_trait;
use est_application::ports::{
    AdiAssessment, AdiRepository, ConversationRepository, DataExporter, EngagementRepository,
    EventLog, GeneratedProgramRecord, GeneratedProgramRepository, LearningProgressRepository,
    LessonProgress, ProjectRepository, RecommendationRepository, RecommendationSet,
    SandboxRunRepository, SessionRepository, StoredEvent, TaskRepository, User, UserRepository,
};
use est_application::{AppError, AppResult};
use est_core::{
    ConversationId, DomainEvent, EscalationId, MessageId, ProjectId, SessionId, TaskId, Timestamp,
    UserId,
};
use est_domain::{
    Card, Conversation, EngagementLevel, EngagementState, Escalation, LanguageDetection,
    LearningSession, Message, NewProject, NewTask, Project, SessionStatus, Task, TaskSource,
    TaskStatus,
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
        // Column `skills` holds the task's ESM discipline ids.
        disciplines: from_json("disciplines", r.get("skills"))?,
        base_revision: r.get("base_revision"),
        test_command: r.get("test_command"),
        acceptance_criteria: from_json("acceptance criteria", r.get("acceptance_criteria"))?,
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
        conversation_id: r
            .get::<Option<String>, _>("conversation_id")
            .map(ConversationId::from),
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
             exercise_id, exercise_version, learning_goals, skills, base_revision, test_command, mentor, docs, created_at, \
             acceptance_criteria) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        .bind(to_json(&t.disciplines))
        .bind(&t.base_revision)
        .bind(&t.test_command)
        .bind(to_json(&t.mentor.clone().unwrap_or_default()))
        .bind(to_json(&t.docs))
        .bind(est_core::now())
        .bind(to_json(
            &t.acceptance_criteria
                .iter()
                .map(|text| est_domain::AcceptanceCriterion { text: text.clone(), done: false })
                .collect::<Vec<_>>(),
        ))
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

    async fn set_acceptance_criteria(
        &self,
        id: &TaskId,
        criteria: &[est_domain::AcceptanceCriterion],
    ) -> AppResult<()> {
        let res = sqlx::query("UPDATE tasks SET acceptance_criteria = ? WHERE id = ?")
            .bind(to_json(&criteria))
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
             base_revision, status, started_at, finished_at, conversation_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        .bind(s.conversation_id.as_ref().map(ConversationId::as_str))
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
            "INSERT INTO session_events (kind, session_id, project_id, task_id, conversation_id, payload, at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&e.kind)
        .bind(e.session_id.as_ref().map(SessionId::as_str))
        .bind(e.project_id.as_ref().map(ProjectId::as_str))
        .bind(e.task_id.as_ref().map(TaskId::as_str))
        .bind(e.conversation_id.as_ref().map(ConversationId::as_str))
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
        self.events_where("session_id", session.as_str(), kind)
            .await
    }

    async fn for_conversation(
        &self,
        conversation: &ConversationId,
        kind: Option<&str>,
    ) -> AppResult<Vec<StoredEvent>> {
        self.events_where("conversation_id", conversation.as_str(), kind)
            .await
    }
}

fn event_from_row(r: &SqliteRow) -> AppResult<StoredEvent> {
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
            conversation_id: r
                .get::<Option<String>, _>("conversation_id")
                .map(ConversationId::from),
            payload: from_json("event payload", r.get("payload"))?,
            at: r.get::<Timestamp, _>("at"),
        },
    })
}

impl SqliteStore {
    async fn events_where(
        &self,
        column: &str,
        value: &str,
        kind: Option<&str>,
    ) -> AppResult<Vec<StoredEvent>> {
        let sql = format!(
            "SELECT * FROM session_events WHERE {column} = ?1 AND (?2 IS NULL OR kind = ?2) ORDER BY id"
        );
        sqlx::query(&sql)
            .bind(value)
            .bind(kind)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?
            .iter()
            .map(event_from_row)
            .collect()
    }
}

// ---- conversations ---------------------------------------------------------

fn level_from(what: &str, v: i64) -> AppResult<EngagementLevel> {
    EngagementLevel::try_from(u8::try_from(v).unwrap_or(u8::MAX)).map_err(|e| corrupt(what, e))
}

fn conversation_from_row(r: &SqliteRow) -> AppResult<Conversation> {
    Ok(Conversation {
        id: ConversationId::from(r.get::<String, _>("id")),
        project_id: r
            .get::<Option<String>, _>("project_id")
            .map(ProjectId::from),
        task_id: r.get::<Option<String>, _>("task_id").map(TaskId::from),
        title: r.get("title"),
        status: parse_enum("conversation status", r.get("status"))?,
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    })
}

fn message_from_row(r: &SqliteRow) -> AppResult<Message> {
    Ok(Message {
        id: MessageId::from(r.get::<String, _>("id")),
        conversation_id: ConversationId::from(r.get::<String, _>("conversation_id")),
        role: parse_enum("message role", r.get("role"))?,
        content: r.get("content"),
        cards: from_json("message cards", r.get("cards"))?,
        level: level_from("message level", r.get("level"))?,
        meta: from_json("message meta", r.get("meta"))?,
        created_at: r.get("created_at"),
    })
}

#[async_trait]
impl ConversationRepository for SqliteStore {
    async fn create(&self, c: &Conversation) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO conversations (id, project_id, task_id, title, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(c.id.as_str())
        .bind(c.project_id.as_ref().map(ProjectId::as_str))
        .bind(c.task_id.as_ref().map(TaskId::as_str))
        .bind(&c.title)
        .bind(enum_str(&c.status))
        .bind(c.created_at)
        .bind(c.updated_at)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    async fn get(&self, id: &ConversationId) -> AppResult<Option<Conversation>> {
        sqlx::query("SELECT * FROM conversations WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .as_ref()
            .map(conversation_from_row)
            .transpose()
    }

    async fn list(&self) -> AppResult<Vec<Conversation>> {
        sqlx::query("SELECT * FROM conversations ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(db)?
            .iter()
            .map(conversation_from_row)
            .collect()
    }

    async fn for_task(&self, task: &TaskId) -> AppResult<Option<Conversation>> {
        sqlx::query("SELECT * FROM conversations WHERE task_id = ?")
            .bind(task.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .as_ref()
            .map(conversation_from_row)
            .transpose()
    }

    async fn append(&self, m: &Message) -> AppResult<()> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query(
            "INSERT INTO conversation_messages (id, conversation_id, role, content, cards, level, meta, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(m.id.as_str())
        .bind(m.conversation_id.as_str())
        .bind(enum_str(&m.role))
        .bind(&m.content)
        .bind(to_json(&m.cards))
        .bind(i64::from(m.level.as_u8()))
        .bind(m.meta.to_string())
        .bind(m.created_at)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        sqlx::query("UPDATE conversations SET updated_at = ? WHERE id = ?")
            .bind(m.created_at)
            .bind(m.conversation_id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)
    }

    async fn message(&self, id: &MessageId) -> AppResult<Option<Message>> {
        sqlx::query("SELECT * FROM conversation_messages WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .as_ref()
            .map(message_from_row)
            .transpose()
    }

    async fn messages(&self, id: &ConversationId, limit: Option<usize>) -> AppResult<Vec<Message>> {
        let limit = limit.map(|l| l as i64).unwrap_or(-1);
        let mut out: Vec<Message> = sqlx::query(
            "SELECT * FROM conversation_messages WHERE conversation_id = ? ORDER BY seq DESC LIMIT ?",
        )
        .bind(id.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?
        .iter()
        .map(message_from_row)
        .collect::<AppResult<_>>()?;
        out.reverse();
        Ok(out)
    }

    async fn update_cards(&self, id: &MessageId, cards: &[Card]) -> AppResult<()> {
        sqlx::query("UPDATE conversation_messages SET cards = ? WHERE id = ?")
            .bind(to_json(&cards))
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(db)?;
        Ok(())
    }

    async fn delete(&self, id: &ConversationId) -> AppResult<()> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let task: Option<Option<String>> =
            sqlx::query_scalar("SELECT task_id FROM conversations WHERE id = ?")
                .bind(id.as_str())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db)?;
        let Some(task) = task else {
            return Err(AppError::NotFound("conversation".into()));
        };
        if let Some(task) = task {
            sqlx::query("DELETE FROM adi_assessments WHERE task_id = ?")
                .bind(&task)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
        }
        for sql in [
            "DELETE FROM session_events WHERE conversation_id = ?",
            "UPDATE learning_sessions SET conversation_id = NULL WHERE conversation_id = ?",
            "DELETE FROM conversations WHERE id = ?",
        ] {
            sqlx::query(sql)
                .bind(id.as_str())
                .execute(&mut *tx)
                .await
                .map_err(db)?;
        }
        tx.commit().await.map_err(db)
    }
}

// ---- engagement ------------------------------------------------------------

fn escalation_from_row(r: &SqliteRow) -> AppResult<Escalation> {
    Ok(Escalation {
        id: EscalationId::from(r.get::<String, _>("id")),
        conversation_id: ConversationId::from(r.get::<String, _>("conversation_id")),
        from: level_from("escalation", r.get("from_level"))?,
        to: level_from("escalation", r.get("to_level"))?,
        required: level_from("escalation", r.get("required_level"))?,
        message_id: MessageId::from(r.get::<String, _>("message_id")),
        status: parse_enum("escalation status", r.get("status"))?,
        created_at: r.get("created_at"),
        resolved_at: r.get("resolved_at"),
    })
}

#[async_trait]
impl EngagementRepository for SqliteStore {
    async fn state(&self, conversation: &ConversationId) -> AppResult<Option<EngagementState>> {
        let Some(r) = sqlx::query("SELECT * FROM engagement_states WHERE conversation_id = ?")
            .bind(conversation.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
        else {
            return Ok(None);
        };
        Ok(Some(EngagementState {
            conversation_id: conversation.clone(),
            level: level_from("engagement level", r.get("level"))?,
            max_level: level_from("engagement max level", r.get("max_level"))?,
            updated_at: r.get("updated_at"),
        }))
    }

    async fn save_state(&self, s: &EngagementState) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO engagement_states (conversation_id, level, max_level, updated_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT(conversation_id) DO UPDATE SET level = excluded.level, \
             max_level = excluded.max_level, updated_at = excluded.updated_at",
        )
        .bind(s.conversation_id.as_str())
        .bind(i64::from(s.level.as_u8()))
        .bind(i64::from(s.max_level.as_u8()))
        .bind(s.updated_at)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    async fn save_escalation(&self, e: &Escalation) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO engagement_escalations (id, conversation_id, from_level, to_level, required_level, \
             message_id, status, created_at, resolved_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET status = excluded.status, resolved_at = excluded.resolved_at",
        )
        .bind(e.id.as_str())
        .bind(e.conversation_id.as_str())
        .bind(i64::from(e.from.as_u8()))
        .bind(i64::from(e.to.as_u8()))
        .bind(i64::from(e.required.as_u8()))
        .bind(e.message_id.as_str())
        .bind(enum_str(&e.status))
        .bind(e.created_at)
        .bind(e.resolved_at)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    async fn escalation(&self, id: &EscalationId) -> AppResult<Option<Escalation>> {
        sqlx::query("SELECT * FROM engagement_escalations WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .as_ref()
            .map(escalation_from_row)
            .transpose()
    }

    async fn escalations(&self, conversation: &ConversationId) -> AppResult<Vec<Escalation>> {
        sqlx::query(
            "SELECT * FROM engagement_escalations WHERE conversation_id = ? ORDER BY created_at, rowid",
        )
        .bind(conversation.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db)?
        .iter()
        .map(escalation_from_row)
        .collect()
    }
}

// ---- ADI -------------------------------------------------------------------

fn adi_from_row(r: &SqliteRow) -> AppResult<AdiAssessment> {
    Ok(AdiAssessment {
        task_id: TaskId::from(r.get::<String, _>("task_id")),
        score: u8::try_from(r.get::<i64, _>("score")).map_err(|e| corrupt("ADI score", e))?,
        scoring_version: r.get("scoring_version"),
        observed_features: from_json("ADI features", r.get("observed_features"))?,
        factors: from_json("ADI factors", r.get("factors"))?,
        explanation: r.get("explanation"),
        confidence: r.get("confidence"),
        created_at: r.get("created_at"),
        sequence: r.get("sequence"),
    })
}

#[async_trait]
impl AdiRepository for SqliteStore {
    async fn save(&self, a: &AdiAssessment) -> AppResult<AdiAssessment> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        // A re-completed task gets a fresh assessment at the end of the order.
        sqlx::query("DELETE FROM adi_assessments WHERE task_id = ?")
            .bind(a.task_id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        let row = sqlx::query(
            "INSERT INTO adi_assessments (task_id, score, scoring_version, observed_features, factors, \
             explanation, confidence, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
        )
        .bind(a.task_id.as_str())
        .bind(i64::from(a.score))
        .bind(&a.scoring_version)
        .bind(a.observed_features.to_string())
        .bind(a.factors.to_string())
        .bind(&a.explanation)
        .bind(&a.confidence)
        .bind(a.created_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        let saved = adi_from_row(&row)?;
        tx.commit().await.map_err(db)?;
        Ok(saved)
    }

    async fn list(&self) -> AppResult<Vec<AdiAssessment>> {
        sqlx::query("SELECT * FROM adi_assessments ORDER BY sequence")
            .fetch_all(&self.pool)
            .await
            .map_err(db)?
            .iter()
            .map(adi_from_row)
            .collect()
    }
}

// ---- sandbox runs ----------------------------------------------------------

fn run_from_row(r: &SqliteRow) -> AppResult<est_domain::SandboxRun> {
    let summary = match (
        r.get::<Option<i64>, _>("passed"),
        r.get::<Option<i64>, _>("failed"),
        r.get::<Option<i64>, _>("ignored"),
    ) {
        (Some(p), Some(f), i) => Some(est_domain::TestSummary {
            passed: p as u32,
            failed: f as u32,
            ignored: i.unwrap_or(0) as u32,
        }),
        _ => None,
    };
    Ok(est_domain::SandboxRun {
        id: r.get("id"),
        session_id: SessionId::from(r.get::<String, _>("session_id")),
        task_id: TaskId::from(r.get::<String, _>("task_id")),
        kind: parse_enum("run kind", r.get("kind"))?,
        command: r.get("command"),
        workdir: r.get("workdir"),
        image: r.get("image"),
        network: r.get::<i64, _>("network") != 0,
        status: parse_enum("run status", r.get("status"))?,
        exit_code: r.get::<Option<i64>, _>("exit_code").map(|c| c as i32),
        summary,
        duration_ms: r.get::<Option<i64>, _>("duration_ms").map(|d| d as u64),
        stdout: r.get("stdout"),
        stderr: r.get("stderr"),
        error: r.get("error"),
        started_at: r.get("started_at"),
        finished_at: r.get("finished_at"),
    })
}

#[async_trait]
impl SandboxRunRepository for SqliteStore {
    async fn create(&self, run: &est_domain::SandboxRun) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO sandbox_runs (id, session_id, task_id, kind, command, workdir, network, status, started_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run.id)
        .bind(run.session_id.as_str())
        .bind(run.task_id.as_str())
        .bind(enum_str(&run.kind))
        .bind(&run.command)
        .bind(&run.workdir)
        .bind(i64::from(run.network))
        .bind(enum_str(&run.status))
        .bind(run.started_at)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    async fn update(&self, run: &est_domain::SandboxRun) -> AppResult<()> {
        sqlx::query(
            "UPDATE sandbox_runs SET image = ?, status = ?, exit_code = ?, passed = ?, failed = ?, ignored = ?, \
             duration_ms = ?, stdout = ?, stderr = ?, error = ?, finished_at = ? WHERE id = ?",
        )
        .bind(&run.image)
        .bind(enum_str(&run.status))
        .bind(run.exit_code.map(i64::from))
        .bind(run.summary.map(|s| i64::from(s.passed)))
        .bind(run.summary.map(|s| i64::from(s.failed)))
        .bind(run.summary.map(|s| i64::from(s.ignored)))
        .bind(run.duration_ms.map(|d| d as i64))
        .bind(&run.stdout)
        .bind(&run.stderr)
        .bind(&run.error)
        .bind(run.finished_at)
        .bind(&run.id)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    async fn get(&self, id: &str) -> AppResult<Option<est_domain::SandboxRun>> {
        sqlx::query("SELECT * FROM sandbox_runs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .as_ref()
            .map(run_from_row)
            .transpose()
    }

    async fn for_session(&self, session: &SessionId) -> AppResult<Vec<est_domain::SandboxRun>> {
        sqlx::query(
            "SELECT * FROM sandbox_runs WHERE session_id = ? ORDER BY started_at DESC, rowid DESC",
        )
        .bind(session.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db)?
        .iter()
        .map(run_from_row)
        .collect()
    }

    async fn fail_interrupted(&self) -> AppResult<u64> {
        Ok(sqlx::query(
            "UPDATE sandbox_runs SET status = 'error', error = 'interrupted by a daemon restart', \
             finished_at = ? WHERE status = 'running'",
        )
        .bind(est_core::now())
        .execute(&self.pool)
        .await
        .map_err(db)?
        .rows_affected())
    }
}

// ---- learning progress -----------------------------------------------------

#[async_trait]
impl LearningProgressRepository for SqliteStore {
    async fn progress(
        &self,
        user: &UserId,
        program: Option<&str>,
    ) -> AppResult<Vec<LessonProgress>> {
        sqlx::query(
            "SELECT * FROM learning_progress WHERE user_id = ?1 AND (?2 IS NULL OR program_id = ?2) \
             ORDER BY program_id, lesson_id",
        )
        .bind(user.as_str())
        .bind(program)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?
        .iter()
        .map(|r| {
            Ok(LessonProgress {
                program_id: r.get("program_id"),
                lesson_id: r.get("lesson_id"),
                status: parse_enum("lesson status", r.get("status"))?,
                score: r.get::<Option<i64>, _>("score").map(|s| s.clamp(0, 100) as u8),
                attempts: r.get::<i64, _>("attempts") as u32,
                detail: from_json("lesson detail", r.get("detail"))?,
                updated_at: r.get("updated_at"),
            })
        })
        .collect()
    }

    async fn save_progress(&self, user: &UserId, p: &LessonProgress) -> AppResult<()> {
        if p.status == est_application::ports::LessonStatus::NotStarted {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO learning_progress (user_id, program_id, lesson_id, status, score, attempts, detail, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(user_id, program_id, lesson_id) DO UPDATE SET \
             status = excluded.status, score = excluded.score, attempts = excluded.attempts, \
             detail = excluded.detail, updated_at = excluded.updated_at",
        )
        .bind(user.as_str())
        .bind(&p.program_id)
        .bind(&p.lesson_id)
        .bind(enum_str(&p.status))
        .bind(p.score.map(i64::from))
        .bind(i64::from(p.attempts))
        .bind(p.detail.to_string())
        .bind(p.updated_at)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }
}

// ---- generated programs ------------------------------------------------------

fn generated_from_row(r: &SqliteRow) -> AppResult<GeneratedProgramRecord> {
    let content: Option<String> = r.get("content");
    Ok(GeneratedProgramRecord {
        id: r.get("id"),
        topic: r.get("topic"),
        lang: r.get("lang"),
        level: parse_enum("program level", r.get("level"))?,
        status: parse_enum("generation status", r.get("status"))?,
        error: r.get("error"),
        lessons_total: r.get::<i64, _>("lessons_total").max(0) as u32,
        lessons_done: r.get::<i64, _>("lessons_done").max(0) as u32,
        model: r.get("model"),
        content: content
            .map(|c| from_json("generated program", &c))
            .transpose()?,
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    })
}

#[async_trait]
impl GeneratedProgramRepository for SqliteStore {
    async fn save_generated(&self, user: &UserId, g: &GeneratedProgramRecord) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO generated_programs (id, user_id, topic, lang, level, status, error, \
             lessons_total, lessons_done, model, content, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET \
             status = excluded.status, error = excluded.error, lessons_total = excluded.lessons_total, \
             lessons_done = excluded.lessons_done, model = excluded.model, content = excluded.content, \
             updated_at = excluded.updated_at",
        )
        .bind(&g.id)
        .bind(user.as_str())
        .bind(&g.topic)
        .bind(&g.lang)
        .bind(enum_str(&g.level))
        .bind(enum_str(&g.status))
        .bind(&g.error)
        .bind(i64::from(g.lessons_total))
        .bind(i64::from(g.lessons_done))
        .bind(&g.model)
        .bind(g.content.as_ref().map(to_json))
        .bind(g.created_at)
        .bind(g.updated_at)
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    async fn generated(&self, id: &str) -> AppResult<Option<GeneratedProgramRecord>> {
        sqlx::query("SELECT * FROM generated_programs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .as_ref()
            .map(generated_from_row)
            .transpose()
    }

    async fn list_generated(&self, user: &UserId) -> AppResult<Vec<GeneratedProgramRecord>> {
        sqlx::query(
            "SELECT * FROM generated_programs WHERE user_id = ? ORDER BY created_at DESC, id",
        )
        .bind(user.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db)?
        .iter()
        .map(generated_from_row)
        .collect()
    }

    async fn delete_generated(&self, user: &UserId, id: &str) -> AppResult<()> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query("DELETE FROM learning_progress WHERE user_id = ? AND program_id = ?")
            .bind(user.as_str())
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("DELETE FROM generated_programs WHERE user_id = ? AND id = ?")
            .bind(user.as_str())
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)
    }

    async fn fail_interrupted_generations(&self) -> AppResult<u64> {
        Ok(sqlx::query(
            "UPDATE generated_programs SET status = 'failed', \
             error = 'interrupted by a daemon restart', updated_at = ? WHERE status = 'generating'",
        )
        .bind(est_core::now())
        .execute(&self.pool)
        .await
        .map_err(db)?
        .rows_affected())
    }
}

// ---- recommendations -----------------------------------------------------------

#[async_trait]
impl RecommendationRepository for SqliteStore {
    async fn save_recommendations(&self, user: &UserId, r: &RecommendationSet) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO recommendations (id, user_id, created_at, model, lang, summary, items, activity) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&r.id)
        .bind(user.as_str())
        .bind(r.created_at)
        .bind(&r.model)
        .bind(&r.lang)
        .bind(&r.summary)
        .bind(to_json(&r.items))
        .bind(r.activity.to_string())
        .execute(&self.pool)
        .await
        .map_err(db)?;
        Ok(())
    }

    async fn recommendations(
        &self,
        user: &UserId,
        limit: usize,
    ) -> AppResult<Vec<RecommendationSet>> {
        sqlx::query(
            "SELECT * FROM recommendations WHERE user_id = ? ORDER BY created_at DESC, rowid DESC LIMIT ?",
        )
        .bind(user.as_str())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(db)?
        .iter()
        .map(|r| {
            Ok(RecommendationSet {
                id: r.get("id"),
                created_at: r.get("created_at"),
                model: r.get("model"),
                lang: r.get("lang"),
                summary: r.get("summary"),
                items: from_json("recommendations", r.get("items"))?,
                activity: from_json("recommendation activity", r.get("activity"))?,
            })
        })
        .collect()
    }
}

// ---- export ----------------------------------------------------------------

#[async_trait]
impl DataExporter for SqliteStore {
    async fn export_all(&self) -> AppResult<serde_json::Value> {
        let user = self.local_user().await?;
        let mut conversations = Vec::new();
        for c in ConversationRepository::list(self).await? {
            conversations.push(serde_json::json!({
                "engagement": self.state(&c.id).await?,
                "escalations": self.escalations(&c.id).await?,
                "messages": self.messages(&c.id, None).await?,
                "conversation": c,
            }));
        }
        let events: Vec<StoredEvent> = sqlx::query("SELECT * FROM session_events ORDER BY id")
            .fetch_all(&self.pool)
            .await
            .map_err(db)?
            .iter()
            .map(event_from_row)
            .collect::<AppResult<_>>()?;
        Ok(serde_json::json!({
            "user": user,
            "projects": ProjectRepository::list(self).await?,
            "tasks": TaskRepository::list(self, None).await?,
            "sessions": SessionRepository::list(self, &user.id).await?,
            "conversations": conversations,
            "events": events,
            "adi_assessments": AdiRepository::list(self).await?,
            "learning_progress": self.progress(&user.id, None).await?,
            // What was asked for and what came out; generated quiz keys stay
            // on the server like bundled ones.
            "recommendations": self.recommendations(&user.id, 1000).await?,
            "generated_programs": self.list_generated(&user.id).await?.iter().map(|g| serde_json::json!({
                "id": g.id, "topic": g.topic, "lang": g.lang, "level": g.level,
                "status": g.status, "model": g.model, "created_at": g.created_at,
                "title": g.content.as_ref().map(|c| c.title.clone()),
                "lessons": g.content.as_ref().map(|c| c.lessons.iter()
                    .map(|l| serde_json::json!({ "id": l.id, "kind": l.kind, "title": l.title }))
                    .collect::<Vec<_>>()),
            })).collect::<Vec<_>>(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use est_domain::{Difficulty, MentorSettings, TaskType};
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
                disciplines: vec!["debugging-testing".into()],
                base_revision: None,
                test_command: Some("cargo test".into()),
                acceptance_criteria: vec!["Tests pass".into()],
                mentor: Some(MentorSettings {
                    max_level: EngagementLevel::Explaining,
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
            conversation_id: None,
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
        assert_eq!(t.mentor.max_level, EngagementLevel::Explaining);
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
        SessionRepository::update(&s, &a).await.unwrap();
        assert!(s.active_for_task(&t.id).await.unwrap().is_none());

        EventLog::append(
            &s,
            &DomainEvent::new("mentor.request", json!({"level": 2})).session(&a.id),
        )
        .await
        .unwrap();
        EventLog::append(
            &s,
            &DomainEvent::new("session.finished", json!({})).session(&a.id),
        )
        .await
        .unwrap();
        EventLog::append(
            &s,
            &DomainEvent::new("project.added", json!({})).project(&p.id),
        )
        .await
        .unwrap();
        assert_eq!(
            EventLog::for_session(&s, &a.id, None).await.unwrap().len(),
            2
        );
        let m = EventLog::for_session(&s, &a.id, Some("mentor.request"))
            .await
            .unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].event.payload["level"], 2);
    }

    #[tokio::test]
    async fn conversations_engagement_adi_and_delete() {
        use est_domain::{ConversationStatus, MessageRole};
        let (s, p, t, user) = seeded().await;
        let now = est_core::now();
        let c = Conversation {
            id: ConversationId::generate(),
            project_id: Some(p.id.clone()),
            task_id: Some(t.id.clone()),
            title: "t".into(),
            status: ConversationStatus::Active,
            created_at: now,
            updated_at: now,
        };
        ConversationRepository::create(&s, &c).await.unwrap();
        assert!(
            ConversationRepository::create(
                &s,
                &Conversation {
                    id: ConversationId::generate(),
                    ..c.clone()
                }
            )
            .await
            .is_err(),
            "one conversation per task"
        );
        assert_eq!(s.for_task(&t.id).await.unwrap().unwrap().id, c.id);

        let m1 = Message::new(&c.id, MessageRole::User, "first", EngagementLevel::Socratic);
        let m2 = Message::new(
            &c.id,
            MessageRole::Assistant,
            "second",
            EngagementLevel::Socratic,
        )
        .with_card(Card::PolicyNotice {
            text: "x".into(),
            code: None,
        });
        ConversationRepository::append(&s, &m1).await.unwrap();
        ConversationRepository::append(&s, &m2).await.unwrap();
        let all = s.messages(&c.id, None).await.unwrap();
        assert_eq!(
            all.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(
            s.messages(&c.id, Some(1)).await.unwrap()[0].content,
            "second"
        );
        assert_eq!(all[1].cards.len(), 1);
        s.update_cards(&m2.id, &[]).await.unwrap();
        assert!(s.message(&m2.id).await.unwrap().unwrap().cards.is_empty());

        let mut state = EngagementState::initial(c.id.clone(), EngagementLevel::Active);
        s.save_state(&state).await.unwrap();
        let mut e = Escalation::propose(
            &state,
            EngagementLevel::Hinting,
            EngagementLevel::Active,
            m1.id.clone(),
        )
        .unwrap();
        s.save_escalation(&e).await.unwrap();
        e.accept(&mut state).unwrap();
        s.save_escalation(&e).await.unwrap();
        s.save_state(&state).await.unwrap();
        assert_eq!(
            s.state(&c.id).await.unwrap().unwrap().level,
            EngagementLevel::Hinting
        );
        assert_eq!(
            s.escalation(&e.id).await.unwrap().unwrap().status,
            est_domain::EscalationStatus::Accepted
        );

        let adi = AdiAssessment {
            task_id: t.id.clone(),
            score: 30,
            scoring_version: "v".into(),
            observed_features: serde_json::json!({}),
            factors: serde_json::json!([]),
            explanation: "e".into(),
            confidence: "low".into(),
            created_at: now,
            sequence: 0,
        };
        let first = AdiRepository::save(&s, &adi).await.unwrap();
        let second = AdiRepository::save(&s, &AdiAssessment { score: 40, ..adi })
            .await
            .unwrap();
        assert!(second.sequence > first.sequence);
        assert_eq!(
            AdiRepository::list(&s).await.unwrap().len(),
            1,
            "one assessment per task"
        );

        let mut session = session(&user, &p, &t, "/ws");
        session.conversation_id = Some(c.id.clone());
        SessionRepository::create(&s, &session).await.unwrap();
        EventLog::append(
            &s,
            &DomainEvent::new("x", serde_json::json!({})).conversation(&c.id),
        )
        .await
        .unwrap();
        assert_eq!(s.for_conversation(&c.id, None).await.unwrap().len(), 1);

        let export = s.export_all().await.unwrap();
        assert_eq!(
            export["conversations"][0]["messages"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        ConversationRepository::delete(&s, &c.id).await.unwrap();
        assert!(
            ConversationRepository::get(&s, &c.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(s.message(&m1.id).await.unwrap().is_none());
        assert!(s.state(&c.id).await.unwrap().is_none());
        assert!(s.for_conversation(&c.id, None).await.unwrap().is_empty());
        assert!(AdiRepository::list(&s).await.unwrap().is_empty());
        assert_eq!(
            SessionRepository::get(&s, &session.id)
                .await
                .unwrap()
                .unwrap()
                .conversation_id,
            None
        );
    }
}
