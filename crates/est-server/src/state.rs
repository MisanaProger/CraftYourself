use std::sync::Arc;

use est_git::UrlPolicy;
use est_storage::{Store, User};

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub config: Arc<Config>,
    /// Local Edition: the implicit user, no registration required.
    pub user: Arc<User>,
}

impl AppState {
    pub async fn init(config: Config) -> anyhow::Result<Self> {
        std::fs::create_dir_all(config.projects_dir())?;
        std::fs::create_dir_all(config.workspaces_dir())?;
        let store = Store::open(&config.db_path()).await?;
        let user = store.ensure_local_user().await?;
        Ok(Self {
            store,
            config: Arc::new(config),
            user: Arc::new(user),
        })
    }

    pub fn url_policy(&self) -> UrlPolicy {
        UrlPolicy {
            allow_local: self.config.allow_local_repos,
        }
    }
}
