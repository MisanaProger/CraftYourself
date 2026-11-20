//! Composition root: builds the application from concrete adapters.

use std::sync::Arc;

use est_application::Ports;
use est_application::ports::EventPublisher;
use est_application::{App, AppSettings};
use est_core::DomainEvent;
use est_git::port::CliGit;
use est_projects::Inspector;
use est_storage::SqliteStore;
use est_workspace::{EditorLauncher, EstHome};
use tokio::sync::broadcast;

use crate::config::Config;

/// Fans domain events out to WebSocket clients.
pub struct BroadcastPublisher(pub broadcast::Sender<DomainEvent>);

impl EventPublisher for BroadcastPublisher {
    fn publish(&self, event: &DomainEvent) {
        // No subscribers is fine.
        let _ = self.0.send(event.clone());
    }
}

#[derive(Clone)]
pub struct Runtime {
    pub app: Arc<App>,
    pub events: broadcast::Sender<DomainEvent>,
    pub home: EstHome,
    pub config: Arc<Config>,
}

impl Runtime {
    /// Initialises `~/.est` (if needed), opens the database and wires ports.
    pub async fn build(home: EstHome, config: Config) -> anyhow::Result<Self> {
        home.init()?;
        let store = Arc::new(SqliteStore::open(&home.database_file()).await?);
        let (events, _) = broadcast::channel(256);
        let ports = Ports {
            users: store.clone(),
            projects: store.clone(),
            tasks: store.clone(),
            sessions: store.clone(),
            events: store,
            publisher: Arc::new(BroadcastPublisher(events.clone())),
            git: Arc::new(CliGit::default()),
            inspector: Arc::new(Inspector::default()),
            workspaces: Arc::new(home.clone()),
            ide: Arc::new(EditorLauncher {
                command: config.editor.clone(),
            }),
        };
        let app = App::new(ports, AppSettings::default()).await?;
        Ok(Self {
            app: Arc::new(app),
            events,
            home,
            config: Arc::new(config),
        })
    }
}
