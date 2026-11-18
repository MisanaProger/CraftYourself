use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// Root for the database, project caches and learner workspaces.
    pub data_dir: PathBuf,
    pub bind: SocketAddr,
    /// Allow cloning `file://` URLs and local paths.
    pub allow_local_repos: bool,
}

impl Config {
    /// Reads configuration from `EST_*` environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        let data_dir = std::env::var_os("EST_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("est-data"));
        let bind = std::env::var("EST_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8080".into())
            .parse()?;
        let allow_local_repos = matches!(
            std::env::var("EST_ALLOW_LOCAL_REPOS").as_deref(),
            Ok("1" | "true" | "yes")
        );
        Ok(Self {
            data_dir,
            bind,
            allow_local_repos,
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("est.db")
    }

    pub fn projects_dir(&self) -> PathBuf {
        self.data_dir.join("projects")
    }

    pub fn workspaces_dir(&self) -> PathBuf {
        self.data_dir.join("workspaces")
    }
}
