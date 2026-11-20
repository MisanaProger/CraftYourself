//! Composition root: builds the application from concrete adapters.

use std::sync::Arc;

use est_application::Ports;
use est_application::ports::{EventPublisher, LlmProvider, LlmSlot, Sandbox};
use est_application::{App, AppSettings};
use est_core::DomainEvent;
use est_git::port::CliGit;
use est_projects::Inspector;
use est_storage::SqliteStore;
use est_workspace::{EditorLauncher, EstHome};
use tokio::sync::broadcast;

use crate::config::Config;
use crate::llm::ProviderRegistry;

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
    /// AI providers connected from the settings page.
    pub providers: Arc<ProviderRegistry>,
}

impl Runtime {
    /// Initialises `~/.est` (if needed), opens the database and wires ports.
    pub async fn build(home: EstHome, config: Config) -> anyhow::Result<Self> {
        home.init()?;
        // The provider picked in the UI wins; `[llm]` from config.toml is
        // used until the learner picks one. An unavailable provider (e.g. a
        // removed key) does not stop the daemon: the mentor reports that no
        // model is connected.
        let llm = ProviderRegistry::new(&home, config.llm.clone()).startup();
        let sandbox: Arc<dyn Sandbox> = Arc::new(est_sandbox::ContainerSandbox::detect(
            config.sandbox.clone(),
        ));
        Self::build_with(home, config, llm, sandbox).await
    }

    /// Like [`Runtime::build`] with an explicit LLM provider (tests, embedding).
    pub async fn build_with(
        home: EstHome,
        config: Config,
        llm: Option<Arc<dyn LlmProvider>>,
        sandbox: Arc<dyn Sandbox>,
    ) -> anyhow::Result<Self> {
        home.init()?;
        let store = Arc::new(SqliteStore::open(&home.database_file()).await?);
        let (events, _) = broadcast::channel(256);
        let ports = Ports {
            users: store.clone(),
            projects: store.clone(),
            tasks: store.clone(),
            sessions: store.clone(),
            events: store.clone(),
            conversations: store.clone(),
            engagement: store.clone(),
            adi: store.clone(),
            export: store.clone(),
            sandbox_runs: store.clone(),
            learning: store,
            sandbox,
            llm: Arc::new(LlmSlot::new(llm)),
            publisher: Arc::new(BroadcastPublisher(events.clone())),
            git: Arc::new(CliGit::default()),
            inspector: Arc::new(Inspector::default()),
            workspaces: Arc::new(home.clone()),
            ide: Arc::new(EditorLauncher {
                command: config.editor.clone(),
            }),
        };
        let settings = AppSettings {
            adi_decay: config.adi.unwrap_or_default(),
        };
        let app = App::new(ports, settings).await?;
        Ok(Self {
            app: Arc::new(app),
            events,
            providers: Arc::new(ProviderRegistry::new(&home, config.llm.clone())),
            home,
            config: Arc::new(config),
        })
    }
}
