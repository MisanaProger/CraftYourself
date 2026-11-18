//! Persistence for the Local Edition (SQLite via SQLx).
//!
//! Queries use runtime-checked SQL so the crate builds without a live
//! database. The schema is kept to portable SQL so a PostgreSQL backend
//! (full Local Edition / Cloud Edition) can be added behind the same API.

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Conflict(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct User {
    pub id: String,
    pub display_name: String,
    pub is_local: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub repository_url: String,
    pub license: Option<String>,
    pub language: Option<String>,
    pub default_branch: Option<String>,
    pub head_revision: String,
    #[serde(skip)]
    pub local_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub description: Option<String>,
    pub repository_url: String,
    pub license: Option<String>,
    pub language: Option<String>,
    pub default_branch: Option<String>,
    pub head_revision: String,
    pub local_path: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ExerciseRecord {
    pub id: String,
    pub version: String,
    pub title: String,
    pub category: String,
    pub difficulty: String,
    pub repository: String,
    pub revision: String,
    pub manifest_toml: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewExercise<'a> {
    pub id: &'a str,
    pub version: &'a str,
    pub title: &'a str,
    pub category: &'a str,
    pub difficulty: &'a str,
    pub repository: &'a str,
    pub revision: &'a str,
    pub manifest_toml: &'a str,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub exercise_id: String,
    pub project_id: String,
    #[serde(skip)]
    pub workspace_path: String,
    pub branch: String,
    pub base_revision: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewSession<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub exercise_id: &'a str,
    pub project_id: &'a str,
    pub workspace_path: &'a str,
    pub branch: &'a str,
    pub base_revision: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Active,
    Completed,
    Abandoned,
}

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Completed => "completed",
            SessionStatus::Abandoned => "abandoned",
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Event {
    pub id: i64,
    pub session_id: String,
    pub kind: String,
    #[sqlx(json)]
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

impl Store {
    /// Opens (creating if needed) a database file and applies migrations.
    pub async fn open(path: &Path) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        Self::connect(opts, 8).await
    }

    /// In-memory database, for tests.
    pub async fn in_memory() -> Result<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // A single connection: every in-memory connection is its own database.
        Self::connect(opts, 1).await
    }

    async fn connect(opts: SqliteConnectOptions, max: u32) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(max)
            .connect_with(opts)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    // ---- users -----------------------------------------------------------

    /// Returns the implicit local user, creating it on first start.
    pub async fn ensure_local_user(&self) -> Result<User> {
        if let Some(u) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE is_local = 1 LIMIT 1")
            .fetch_optional(&self.pool)
            .await?
        {
            return Ok(u);
        }
        let user = User {
            id: new_id(),
            display_name: "Local learner".into(),
            is_local: true,
            created_at: Utc::now(),
        };
        sqlx::query(
            "INSERT INTO users (id, display_name, is_local, created_at) VALUES (?, ?, 1, ?)",
        )
        .bind(&user.id)
        .bind(&user.display_name)
        .bind(user.created_at)
        .execute(&self.pool)
        .await?;
        Ok(user)
    }

    // ---- projects --------------------------------------------------------

    pub async fn insert_project(&self, p: NewProject) -> Result<Project> {
        let id = new_id();
        let res = sqlx::query(
            "INSERT INTO projects (id, name, description, repository_url, license, language, \
             default_branch, head_revision, local_path, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&p.name)
        .bind(&p.description)
        .bind(&p.repository_url)
        .bind(&p.license)
        .bind(&p.language)
        .bind(&p.default_branch)
        .bind(&p.head_revision)
        .bind(&p.local_path)
        .bind(Utc::now())
        .execute(&self.pool)
        .await;
        match res {
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                return Err(StorageError::Conflict(format!(
                    "project for {} already exists",
                    p.repository_url
                )))
            }
            other => {
                other?;
            }
        }
        self.get_project(&id).await
    }

    pub async fn get_project(&self, id: &str) -> Result<Project> {
        sqlx::query_as("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StorageError::NotFound("project"))
    }

    pub async fn find_project_by_url(&self, url: &str) -> Result<Option<Project>> {
        Ok(
            sqlx::query_as("SELECT * FROM projects WHERE repository_url = ?")
                .bind(url)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        Ok(
            sqlx::query_as("SELECT * FROM projects ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    // ---- exercises -------------------------------------------------------

    /// Inserts a new exercise or replaces an existing one with the same id.
    pub async fn upsert_exercise(&self, e: NewExercise<'_>) -> Result<ExerciseRecord> {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO exercises (id, version, title, category, difficulty, repository, revision, \
             manifest_toml, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET version = excluded.version, title = excluded.title, \
             category = excluded.category, difficulty = excluded.difficulty, \
             repository = excluded.repository, revision = excluded.revision, \
             manifest_toml = excluded.manifest_toml, updated_at = excluded.updated_at",
        )
        .bind(e.id)
        .bind(e.version)
        .bind(e.title)
        .bind(e.category)
        .bind(e.difficulty)
        .bind(e.repository)
        .bind(e.revision)
        .bind(e.manifest_toml)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_exercise(e.id).await
    }

    pub async fn get_exercise(&self, id: &str) -> Result<ExerciseRecord> {
        sqlx::query_as("SELECT * FROM exercises WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StorageError::NotFound("exercise"))
    }

    pub async fn list_exercises(&self) -> Result<Vec<ExerciseRecord>> {
        Ok(sqlx::query_as("SELECT * FROM exercises ORDER BY title")
            .fetch_all(&self.pool)
            .await?)
    }

    // ---- sessions --------------------------------------------------------

    pub async fn create_session(&self, s: NewSession<'_>) -> Result<Session> {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO sessions (id, user_id, exercise_id, project_id, workspace_path, branch, \
             base_revision, status, started_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)",
        )
        .bind(s.id)
        .bind(s.user_id)
        .bind(s.exercise_id)
        .bind(s.project_id)
        .bind(s.workspace_path)
        .bind(s.branch)
        .bind(s.base_revision)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_session(s.id).await
    }

    pub async fn get_session(&self, id: &str) -> Result<Session> {
        sqlx::query_as("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StorageError::NotFound("session"))
    }

    pub async fn list_sessions(&self, user_id: &str) -> Result<Vec<Session>> {
        Ok(
            sqlx::query_as("SELECT * FROM sessions WHERE user_id = ? ORDER BY updated_at DESC")
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn set_session_status(&self, id: &str, status: SessionStatus) -> Result<Session> {
        let res = sqlx::query("UPDATE sessions SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(StorageError::NotFound("session"));
        }
        self.get_session(id).await
    }

    // ---- events ----------------------------------------------------------

    pub async fn record_event(
        &self,
        session_id: &str,
        kind: &str,
        payload: &serde_json::Value,
    ) -> Result<i64> {
        let now = Utc::now();
        let id = sqlx::query(
            "INSERT INTO events (session_id, kind, payload, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(session_id)
        .bind(kind)
        .bind(payload.to_string())
        .bind(now)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        sqlx::query("UPDATE sessions SET updated_at = ? WHERE id = ?")
            .bind(now)
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn list_events(&self, session_id: &str) -> Result<Vec<Event>> {
        Ok(
            sqlx::query_as("SELECT * FROM events WHERE session_id = ? ORDER BY id")
                .bind(session_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn count_events(&self, session_id: &str, kinds: &[&str]) -> Result<u32> {
        let mut total = 0u32;
        for kind in kinds {
            let (n,): (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM events WHERE session_id = ? AND kind = ?")
                    .bind(session_id)
                    .bind(kind)
                    .fetch_one(&self.pool)
                    .await?;
            total += u32::try_from(n).unwrap_or(u32::MAX);
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seeded() -> (Store, User, Project, ExerciseRecord) {
        let store = Store::in_memory().await.unwrap();
        let user = store.ensure_local_user().await.unwrap();
        let project = store
            .insert_project(NewProject {
                name: "ranges".into(),
                description: None,
                repository_url: "https://github.com/example/ranges".into(),
                license: Some("MIT".into()),
                language: Some("rust".into()),
                default_branch: Some("main".into()),
                head_revision: "a".repeat(40),
                local_path: "/tmp/ranges".into(),
            })
            .await
            .unwrap();
        let ex = store
            .upsert_exercise(NewExercise {
                id: "fix-off-by-one",
                version: "1.0.0",
                title: "Fix off-by-one",
                category: "bug-fix",
                difficulty: "beginner",
                repository: &project.repository_url,
                revision: &"a".repeat(40),
                manifest_toml: "manifest_version = 1",
            })
            .await
            .unwrap();
        (store, user, project, ex)
    }

    #[tokio::test]
    async fn local_user_is_singleton() {
        let store = Store::in_memory().await.unwrap();
        let a = store.ensure_local_user().await.unwrap();
        let b = store.ensure_local_user().await.unwrap();
        assert_eq!(a.id, b.id);
        assert!(a.is_local);
    }

    #[tokio::test]
    async fn project_urls_are_unique() {
        let (store, _, p, _) = seeded().await;
        let dup = store
            .insert_project(NewProject {
                name: "x".into(),
                description: None,
                repository_url: p.repository_url.clone(),
                license: None,
                language: None,
                default_branch: None,
                head_revision: "b".repeat(40),
                local_path: "/tmp/x".into(),
            })
            .await;
        assert!(matches!(dup, Err(StorageError::Conflict(_))));
        assert_eq!(
            store
                .find_project_by_url(&p.repository_url)
                .await
                .unwrap()
                .unwrap()
                .id,
            p.id
        );
    }

    #[tokio::test]
    async fn exercise_upsert_updates_version() {
        let (store, _, _, ex) = seeded().await;
        let rev = "a".repeat(40);
        let updated = store
            .upsert_exercise(NewExercise {
                id: &ex.id,
                version: "1.1.0",
                title: &ex.title,
                category: &ex.category,
                difficulty: &ex.difficulty,
                repository: &ex.repository,
                revision: &rev,
                manifest_toml: &ex.manifest_toml,
            })
            .await
            .unwrap();
        assert_eq!(updated.version, "1.1.0");
        assert_eq!(updated.created_at, ex.created_at);
        assert_eq!(store.list_exercises().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn sessions_and_events() {
        let (store, user, project, ex) = seeded().await;
        let sid = new_id();
        let s = store
            .create_session(NewSession {
                id: &sid,
                user_id: &user.id,
                exercise_id: &ex.id,
                project_id: &project.id,
                workspace_path: "/tmp/ws",
                branch: "est/fix-off-by-one",
                base_revision: &"a".repeat(40),
            })
            .await
            .unwrap();
        assert_eq!(s.status, "active");

        store
            .record_event(&sid, "mentor.hint", &serde_json::json!({"level": 2}))
            .await
            .unwrap();
        store
            .record_event(&sid, "git.commit", &serde_json::json!({"hash": "x"}))
            .await
            .unwrap();
        store
            .record_event(&sid, "git.commit", &serde_json::json!({"hash": "y"}))
            .await
            .unwrap();

        let events = store.list_events(&sid).await.unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].payload["level"], 2);
        assert_eq!(store.count_events(&sid, &["git.commit"]).await.unwrap(), 2);
        assert_eq!(
            store
                .count_events(&sid, &["git.commit", "mentor.hint"])
                .await
                .unwrap(),
            3
        );

        let done = store
            .set_session_status(&sid, SessionStatus::Completed)
            .await
            .unwrap();
        assert_eq!(done.status, "completed");
        assert_eq!(store.list_sessions(&user.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn events_require_existing_session() {
        let store = Store::in_memory().await.unwrap();
        assert!(store
            .record_event("nope", "x", &serde_json::json!({}))
            .await
            .is_err());
    }
}
