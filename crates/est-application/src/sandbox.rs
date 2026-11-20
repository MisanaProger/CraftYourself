//! Running build/test/check commands in the Sandbox Manager (v0.3 §17).
//!
//! Runs execute asynchronously: `start_run` records the run as `running`,
//! returns at once and finishes in the background; the result lands in the
//! database, the event log and the Issue's conversation as a card.

use est_core::{DomainEvent, SessionId};
use est_domain::{
    Card, CommandKind, LearningSession, Message, MessageRole, RunKind, RunStatus, SandboxRun,
};
use serde::Deserialize;
use serde_json::json;

use crate::ports::{SandboxInfo, SandboxRequest};
use crate::service::blocking;
use crate::{App, AppError, AppResult};

pub mod events {
    pub const RUN_STARTED: &str = "sandbox.run.started";
    pub const RUN_FINISHED: &str = "sandbox.run.finished";
    pub const NETWORK_GRANTED: &str = "permission.network.granted";
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RunRequest {
    #[serde(default)]
    pub kind: RunKind,
    /// Explicit command line; otherwise the task's or the adapters' command.
    #[serde(default)]
    pub command: Option<String>,
    /// Relative working directory for an explicit command.
    #[serde(default)]
    pub workdir: Option<String>,
    /// The learner's explicit permission for network access in this run
    /// (a permission gate, v0.3 §7.5). Never implied.
    #[serde(default)]
    pub allow_network: bool,
}

struct Resolved {
    command: String,
    workdir: String,
    language: Option<String>,
}

impl App {
    pub fn sandbox_info(&self) -> SandboxInfo {
        self.ports.sandbox.info()
    }

    async fn resolve_command(&self, s: &LearningSession, req: &RunRequest) -> AppResult<Resolved> {
        let task = self.get_task(&s.task_id).await?;
        let project = self.get_project(&s.project_id).await?;
        if let Some(cmd) = req
            .command
            .as_ref()
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
        {
            if cmd.len() > 2000 {
                return Err(AppError::Invalid("command is too long".into()));
            }
            return Ok(Resolved {
                command: cmd.to_owned(),
                workdir: req.workdir.clone().unwrap_or_default(),
                language: project.primary_language.clone(),
            });
        }
        let want = match req.kind {
            RunKind::Test => CommandKind::Test,
            RunKind::Build => CommandKind::Build,
            RunKind::Check => CommandKind::Check,
            RunKind::Setup => {
                let command = match project.primary_language.as_deref() {
                    Some("rust") => "cargo fetch",
                    Some("python") => {
                        "if [ -f requirements.txt ]; then pip install --target /deps/python -r requirements.txt; fi"
                    }
                    _ => {
                        return Err(AppError::Invalid(
                            "no default setup command for this language; pass `command`".into(),
                        ));
                    }
                };
                return Ok(Resolved {
                    command: command.into(),
                    workdir: String::new(),
                    language: project.primary_language,
                });
            }
            RunKind::Custom => {
                return Err(AppError::Invalid("a custom run needs `command`".into()));
            }
        };
        if want == CommandKind::Test
            && let Some(cmd) = &task.test_command
        {
            return Ok(Resolved {
                command: cmd.clone(),
                workdir: String::new(),
                language: project.primary_language,
            });
        }
        let ws = s.workspace_path.clone();
        let (git, inspector) = (self.ports.git.clone(), self.ports.inspector.clone());
        let specs = blocking(move || {
            let files = git.tracked_files(&ws)?;
            Ok(inspector.commands(&ws, &files))
        })
        .await?;
        let matching: Vec<_> = specs.into_iter().filter(|c| c.kind == want).collect();
        let first = matching.first().ok_or_else(|| {
            AppError::Invalid(
                format!("no {:?} command found for this project", req.kind).to_lowercase(),
            )
        })?;
        // Several steps of one language (configure + build) run in order.
        let steps: Vec<String> = matching
            .iter()
            .filter(|c| c.language == first.language && c.workdir == first.workdir)
            .map(|c| c.display())
            .collect();
        Ok(Resolved {
            command: steps.join(" && "),
            workdir: first.workdir.clone(),
            language: Some(first.language.clone()),
        })
    }

    /// Records a run and executes it in the background.
    pub async fn start_run(
        &self,
        session_id: &SessionId,
        req: RunRequest,
    ) -> AppResult<SandboxRun> {
        let (run, request) = self.prepare_run(session_id, &req).await?;
        let app = self.clone();
        let r = run.clone();
        tokio::spawn(async move {
            if let Err(e) = app.execute_run(r, request).await {
                tracing_log(&e);
            }
        });
        Ok(run)
    }

    /// Records a run and waits for its result (CLI, tests).
    pub async fn run_and_wait(
        &self,
        session_id: &SessionId,
        req: RunRequest,
    ) -> AppResult<SandboxRun> {
        let (run, request) = self.prepare_run(session_id, &req).await?;
        self.execute_run(run, request).await
    }

    async fn prepare_run(
        &self,
        session_id: &SessionId,
        req: &RunRequest,
    ) -> AppResult<(SandboxRun, SandboxRequest)> {
        let session = self.get_session(session_id).await?;
        if !session.is_active() {
            return Err(AppError::Conflict("the session is finished".into()));
        }
        let info = self.ports.sandbox.info();
        if !info.available {
            return Err(AppError::Conflict(format!(
                "sandbox unavailable: {}",
                info.reason.unwrap_or_else(|| "no container runtime".into())
            )));
        }
        if self
            .ports
            .sandbox_runs
            .for_session(session_id)
            .await?
            .iter()
            .any(|r| r.status == RunStatus::Running)
        {
            return Err(AppError::Conflict(
                "another run is in progress in this session".into(),
            ));
        }
        if req.kind == RunKind::Setup && !req.allow_network {
            return Err(AppError::Invalid(
                "fetching dependencies needs network access: confirm it with `allow_network`"
                    .into(),
            ));
        }
        let resolved = self.resolve_command(&session, req).await?;
        let run = SandboxRun {
            id: est_core::SessionId::generate().to_string(),
            session_id: session.id.clone(),
            task_id: session.task_id.clone(),
            kind: req.kind,
            command: resolved.command.clone(),
            workdir: resolved.workdir.clone(),
            image: None,
            network: req.allow_network,
            status: RunStatus::Running,
            exit_code: None,
            summary: None,
            duration_ms: None,
            stdout: String::new(),
            stderr: String::new(),
            error: None,
            started_at: est_core::now(),
            finished_at: None,
        };
        self.ports.sandbox_runs.create(&run).await?;
        let mut ev = DomainEvent::new(
            events::RUN_STARTED,
            json!({ "run_id": run.id, "kind": run.kind, "command": run.command, "network": run.network }),
        )
        .session(&session.id)
        .task(&session.task_id);
        if let Some(c) = &session.conversation_id {
            ev = ev.conversation(c);
        }
        self.emit(ev).await?;
        if req.allow_network {
            self.emit(
                DomainEvent::new(
                    events::NETWORK_GRANTED,
                    json!({ "run_id": run.id, "command": run.command }),
                )
                .session(&session.id),
            )
            .await?;
        }
        let request = SandboxRequest {
            run_id: run.id.clone(),
            workspace: session.workspace_path.clone(),
            workdir: resolved.workdir,
            command: resolved.command,
            language: resolved.language,
            network: req.allow_network,
            cache_key: session.id.to_string(),
            deps_key: session.project_id.to_string(),
        };
        Ok((run, request))
    }

    async fn execute_run(
        &self,
        mut run: SandboxRun,
        request: SandboxRequest,
    ) -> AppResult<SandboxRun> {
        match self.ports.sandbox.run(request).await {
            Ok(out) => {
                run.summary = if matches!(run.kind, RunKind::Test | RunKind::Custom) {
                    self.ports
                        .inspector
                        .summarize_tests(&format!("{}\n{}", out.stdout, out.stderr))
                } else {
                    None
                };
                run.status = if out.timed_out {
                    RunStatus::TimedOut
                } else if out.exit_code == Some(0) {
                    RunStatus::Passed
                } else {
                    RunStatus::Failed
                };
                run.exit_code = out.exit_code;
                run.duration_ms = Some(out.duration_ms);
                run.image = Some(out.image);
                run.stdout = out.stdout;
                run.stderr = out.stderr;
            }
            Err(e) => {
                run.status = RunStatus::Error;
                run.error = Some(e.to_string());
            }
        }
        run.finished_at = Some(est_core::now());
        // The result goes to the chat before the run is marked finished, so
        // whoever sees a finished run also finds its card in the timeline.
        // The run is closed even if posting fails.
        let posted = self.post_run_result(&run).await;
        self.ports.sandbox_runs.update(&run).await?;
        let ev = posted?;
        self.emit(ev).await?;
        Ok(run)
    }

    /// Adds the run's result to its Issue's conversation; returns the event
    /// to emit once the run is stored.
    async fn post_run_result(&self, run: &SandboxRun) -> AppResult<DomainEvent> {
        let session = self.get_session(&run.session_id).await?;
        let mut ev = DomainEvent::new(
            events::RUN_FINISHED,
            json!({
                "run_id": run.id, "kind": run.kind, "status": run.status,
                "summary": run.summary, "exit_code": run.exit_code, "duration_ms": run.duration_ms,
            }),
        )
        .session(&session.id)
        .task(&session.task_id);
        if let Some(c) = &session.conversation_id {
            ev = ev.conversation(c);
            // The result appears in the chat timeline (v0.3 §6.3).
            let level = self.engagement(c).await?.level;
            let text = match (&run.status, &run.summary) {
                (RunStatus::Error, _) => format!(
                    "Запуск не удался: {}",
                    run.error.clone().unwrap_or_default()
                ),
                (_, Some(s)) => format!(
                    "`{}`: {} passed · {} failed",
                    run.command, s.passed, s.failed
                ),
                (status, None) => format!("`{}`: {status:?}", run.command),
            };
            let msg = Message::new(c, MessageRole::System, text, level)
                .with_card(Card::SandboxRun {
                    run_id: run.id.clone(),
                    kind: run.kind,
                    command: run.command.clone(),
                    status: run.status,
                    summary: run.summary,
                    exit_code: run.exit_code,
                    duration_ms: run.duration_ms,
                })
                .with_meta(json!({ "notice": "sandbox_run" }));
            self.ports.conversations.append(&msg).await?;
        }
        Ok(ev)
    }

    pub async fn runs(&self, session_id: &SessionId) -> AppResult<Vec<SandboxRun>> {
        self.get_session(session_id).await?;
        self.ports.sandbox_runs.for_session(session_id).await
    }

    pub async fn get_run(&self, id: &str) -> AppResult<SandboxRun> {
        let run = self
            .ports
            .sandbox_runs
            .get(id)
            .await?
            .ok_or_else(|| AppError::NotFound("sandbox run".into()))?;
        self.get_session(&run.session_id).await?;
        Ok(run)
    }
}

fn tracing_log(e: &AppError) {
    tracing::error!("sandbox run failed: {e}");
}
