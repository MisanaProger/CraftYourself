//! `est` — command line interface of Engineering Skills Trainer
//! (architecture v0.2, §7.2). Works directly with the local database, so it
//! is usable with or without the daemon (`est serve`) running.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use est_api::{Config, Runtime};
use est_application::{AddProject, App, MentorAsk};
use est_core::TaskId;
use est_domain::{
    Difficulty, HelpLevel, LearningSession, NewTask, SessionOutcome, TaskSource, TaskType,
};
use est_workspace::EstHome;

#[derive(Parser)]
#[command(
    name = "est",
    version,
    about = "Engineering Skills Trainer — learn by working on real code"
)]
struct Cli {
    /// EST data directory (default: $EST_HOME or ~/.est).
    #[arg(long, global = true, env = "EST_HOME")]
    home: Option<PathBuf>,
    /// Print machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create ~/.est and a default configuration.
    Init,
    /// Run the local daemon: REST API, WebSocket and the web interface.
    Serve,
    /// Connect and inspect projects.
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Create, import and start learning tasks.
    #[command(subcommand)]
    Task(TaskCmd),
    /// The current task's working directory.
    #[command(subcommand)]
    Workspace(WorkspaceCmd),
    /// Learning sessions.
    #[command(subcommand)]
    Session(SessionCmd),
    /// Ask the AI mentor (subject to the AI Policy Engine).
    #[command(subcommand)]
    Mentor(MentorCmd),
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// Connect a local Git repository (path) or clone a remote one (URL).
    Add {
        source: String,
    },
    List,
    /// Languages and suggested build/test/check commands.
    Show {
        id: String,
    },
}

#[derive(Subcommand)]
enum TaskCmd {
    List {
        #[arg(long)]
        project: Option<String>,
    },
    Create(CreateTask),
    /// Import an Exercise Manifest (TOML).
    Import {
        manifest: PathBuf,
    },
    /// Start (or resume) a task in its own workspace.
    Start {
        task: String,
        /// Also open the workspace in the editor.
        #[arg(long)]
        open: bool,
    },
}

#[derive(Args)]
struct CreateTask {
    #[arg(long)]
    project: String,
    #[arg(long)]
    title: String,
    #[arg(long, default_value = "")]
    description: String,
    /// bug-fix, feature, refactoring, documentation, performance, testing
    #[arg(long = "type", default_value = "bug-fix")]
    task_type: String,
    /// beginner, intermediate, advanced
    #[arg(long, default_value = "beginner")]
    difficulty: String,
    /// Pin the task to a commit (default: HEAD when started).
    #[arg(long)]
    base: Option<String>,
}

#[derive(Args, Clone)]
struct SessionSel {
    /// Session id (or prefix). Default: the session whose workspace contains
    /// the current directory.
    #[arg(long)]
    session: Option<String>,
}

#[derive(Subcommand)]
enum WorkspaceCmd {
    /// Open the workspace in the configured editor.
    Open(SessionSel),
    /// Changes and own commits in the workspace.
    Status(SessionSel),
    /// Print the workspace path (`cd "$(est workspace path)"`).
    Path(SessionSel),
}

#[derive(Subcommand)]
enum SessionCmd {
    List,
    Status(SessionSel),
    Finish {
        #[command(flatten)]
        sel: SessionSel,
        /// Give up instead of completing.
        #[arg(long)]
        abandon: bool,
    },
}

#[derive(Subcommand)]
enum MentorCmd {
    Ask {
        question: String,
        /// Requested help level 0–4.
        #[arg(long, default_value_t = 1)]
        level: u8,
        /// Documentation fragment with terms to explain.
        #[arg(long)]
        excerpt: Option<String>,
        #[command(flatten)]
        sel: SessionSel,
    },
    History(SessionSel),
}

fn parse_kebab<T: serde::de::DeserializeOwned>(what: &str, s: &str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(s.to_owned()))
        .map_err(|_| anyhow!("unknown {what} `{s}`"))
}

fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

struct Ctx {
    app: std::sync::Arc<App>,
    json: bool,
}

impl Ctx {
    /// Accepts full ids or unique prefixes, like git.
    async fn task_id(&self, arg: &str) -> Result<TaskId> {
        let all = self.app.list_tasks(None).await?;
        let hits: Vec<_> = all
            .iter()
            .filter(|t| t.id.as_str().starts_with(arg))
            .collect();
        match hits.as_slice() {
            [one] => Ok(one.id.clone()),
            [] => bail!("no task matches `{arg}`"),
            _ => bail!("`{arg}` matches several tasks; use more characters"),
        }
    }

    async fn session(&self, sel: &SessionSel) -> Result<LearningSession> {
        if let Some(arg) = &sel.session {
            let all = self.app.list_sessions().await?;
            let hits: Vec<_> = all
                .into_iter()
                .filter(|s| s.id.as_str().starts_with(arg.as_str()))
                .collect();
            return match hits.len() {
                1 => Ok(hits.into_iter().next().unwrap()),
                0 => bail!("no session matches `{arg}`"),
                _ => bail!("`{arg}` matches several sessions; use more characters"),
            };
        }
        let cwd = std::env::current_dir()?;
        self.app.session_for_path(&cwd).await.map_err(|_| {
            anyhow!("not inside an EST workspace: run this in a task's workspace or pass --session")
        })
    }
}

fn load_config(home: &EstHome) -> Result<Config> {
    Config::load(&home.config_file())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let home = cli
        .home
        .clone()
        .map(EstHome::new)
        .unwrap_or_else(EstHome::from_env);

    match &cli.command {
        Command::Init => return init(&home),
        Command::Serve => return serve(home).await,
        _ => {}
    }

    let rt = Runtime::build(home.clone(), load_config(&home)?).await?;
    let ctx = Ctx {
        app: rt.app.clone(),
        json: cli.json,
    };
    run(&ctx, cli.command).await
}

fn init(home: &EstHome) -> Result<()> {
    home.init()?;
    let cfg = home.config_file();
    if !cfg.exists() {
        std::fs::write(&cfg, Config::TEMPLATE)?;
    }
    println!("EST home: {}", home.root().display());
    println!("config:   {}", cfg.display());
    Ok(())
}

async fn serve(home: EstHome) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config = load_config(&home)?;
    let bind = config.bind;
    let frontend = config.frontend_dir.clone().or_else(|| {
        Some(PathBuf::from("frontend/dist")).filter(|p| p.join("index.html").is_file())
    });
    let rt = Runtime::build(home.clone(), config).await?;
    tracing::info!(home = %home.root().display(), "starting EST daemon");
    match &frontend {
        Some(dir) => tracing::info!(frontend = %dir.display(), "serving web interface"),
        None => tracing::info!("web interface not built; API only (see frontend/README.md)"),
    }
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("cannot listen on {bind}"))?;
    tracing::info!("listening on http://{}", listener.local_addr()?);
    est_api::serve(listener, rt, frontend.as_deref()).await
}

fn session_line(s: &LearningSession) -> String {
    format!(
        "{}  {:<9}  {}  {}",
        s.id.short(),
        s.status.as_str(),
        s.branch,
        s.workspace_path.display()
    )
}

async fn run(ctx: &Ctx, command: Command) -> Result<()> {
    let app = &ctx.app;
    match command {
        Command::Init | Command::Serve => unreachable!("handled before the runtime starts"),

        Command::Project(ProjectCmd::Add { source }) => {
            // Relative paths are relative to the caller, not the daemon.
            let source = if Path::new(&source).is_dir() {
                std::fs::canonicalize(&source)?
                    .to_string_lossy()
                    .into_owned()
            } else {
                source
            };
            let p = app.add_project(AddProject { source }).await?;
            if ctx.json {
                return print_json(&p);
            }
            println!("connected {} ({})", p.name, p.id.short());
            println!("  path:      {}", p.local_path.display());
            let langs: Vec<_> = p
                .languages
                .iter()
                .map(|l| format!("{} ({} files)", l.language, l.files))
                .collect();
            println!(
                "  languages: {}",
                if langs.is_empty() {
                    "none detected".into()
                } else {
                    langs.join(", ")
                }
            );
            match &p.license {
                Some(l) => println!("  license:   {l}"),
                None => println!(
                    "  license:   none detected — the code is all rights reserved; study it locally only"
                ),
            }
        }
        Command::Project(ProjectCmd::List) => {
            let ps = app.list_projects().await?;
            if ctx.json {
                return print_json(&ps);
            }
            for p in ps {
                println!(
                    "{}  {:<20} {:<10} {}",
                    p.id.short(),
                    p.name,
                    p.primary_language.as_deref().unwrap_or("-"),
                    p.local_path.display()
                );
            }
        }
        Command::Project(ProjectCmd::Show { id }) => {
            let ps = app.list_projects().await?;
            let p = ps
                .into_iter()
                .find(|p| p.id.as_str().starts_with(&id))
                .ok_or_else(|| anyhow!("no project matches `{id}`"))?;
            let cmds = app.project_commands(&p.id).await?;
            if ctx.json {
                return print_json(&serde_json::json!({ "project": p, "commands": cmds }));
            }
            println!("{} — {}", p.name, p.local_path.display());
            for l in &p.languages {
                println!(
                    "  {:<10} {:>5} files  {}",
                    l.language,
                    l.files,
                    l.build_systems.join(", ")
                );
            }
            if !cmds.is_empty() {
                println!("suggested commands (run in the sandbox, stage 2):");
                for c in cmds {
                    let dir = if c.workdir.is_empty() {
                        ".".into()
                    } else {
                        c.workdir.clone()
                    };
                    println!(
                        "  [{:?}] {:<40} in {dir}  — {}",
                        c.kind,
                        c.display(),
                        c.description
                    );
                }
            }
        }

        Command::Task(TaskCmd::List { project }) => {
            let project = match project {
                Some(p) => Some(
                    app.list_projects()
                        .await?
                        .into_iter()
                        .find(|x| x.id.as_str().starts_with(&p))
                        .ok_or_else(|| anyhow!("no project matches `{p}`"))?
                        .id,
                ),
                None => None,
            };
            let tasks = app.list_tasks(project.as_ref()).await?;
            if ctx.json {
                return print_json(&tasks);
            }
            for t in tasks {
                let src = match &t.source {
                    TaskSource::Exercise { id, .. } => format!("exercise:{id}"),
                    TaskSource::Issue { url } => url.clone(),
                    TaskSource::Manual => "manual".into(),
                };
                println!(
                    "{}  {:<12} {:<12} {:<12} {}  [{src}]",
                    t.id.short(),
                    format!("{:?}", t.status).to_lowercase(),
                    format!("{:?}", t.task_type).to_lowercase(),
                    format!("{:?}", t.difficulty).to_lowercase(),
                    t.title
                );
            }
        }
        Command::Task(TaskCmd::Create(c)) => {
            let project = app
                .list_projects()
                .await?
                .into_iter()
                .find(|x| x.id.as_str().starts_with(&c.project))
                .ok_or_else(|| anyhow!("no project matches `{}`", c.project))?;
            let task = app
                .create_task(NewTask {
                    project_id: project.id,
                    title: c.title,
                    description: c.description,
                    task_type: parse_kebab::<TaskType>("task type", &c.task_type)?,
                    difficulty: parse_kebab::<Difficulty>("difficulty", &c.difficulty)?,
                    source: TaskSource::Manual,
                    learning_goals: vec![],
                    skills: vec![],
                    base_revision: c.base,
                    test_command: None,
                    mentor: None,
                    docs: vec![],
                })
                .await?;
            if ctx.json {
                return print_json(&task);
            }
            println!("created task {} — {}", task.id.short(), task.title);
            println!("start it with: est task start {}", task.id.short());
        }
        Command::Task(TaskCmd::Import { manifest }) => {
            let text = std::fs::read_to_string(&manifest)
                .with_context(|| format!("cannot read {}", manifest.display()))?;
            let task = app.import_exercise(&text).await?;
            if ctx.json {
                return print_json(&task);
            }
            println!("task {} — {}", task.id.short(), task.title);
        }
        Command::Task(TaskCmd::Start { task, open }) => {
            let id = ctx.task_id(&task).await?;
            let started = app.start_task(&id).await?;
            if open {
                app.open_workspace(&started.session.id).await?;
            }
            if ctx.json {
                return print_json(&started);
            }
            let s = &started.session;
            println!(
                "{} session {}",
                if started.resumed {
                    "resumed"
                } else {
                    "started"
                },
                s.id.short()
            );
            println!("  workspace: {}", s.workspace_path.display());
            println!("  branch:    {}", s.branch);
            println!(
                "open it in your editor (`est workspace open`) and commit your work as usual."
            );
        }

        Command::Workspace(WorkspaceCmd::Open(sel)) => {
            let s = ctx.session(&sel).await?;
            let cmd = app.open_workspace(&s.id).await?;
            println!("opened: {cmd}");
        }
        Command::Workspace(WorkspaceCmd::Path(sel)) => {
            println!("{}", ctx.session(&sel).await?.workspace_path.display());
        }
        Command::Workspace(WorkspaceCmd::Status(sel))
        | Command::Session(SessionCmd::Status(sel)) => {
            let s = ctx.session(&sel).await?;
            let o = app.session_overview(&s.id).await?;
            if ctx.json {
                return print_json(&o);
            }
            println!("task:      {}", o.task.title);
            println!(
                "session:   {} ({})",
                o.session.id.short(),
                o.session.status.as_str()
            );
            println!("workspace: {}", o.session.workspace_path.display());
            println!(
                "branch:    {}",
                o.current_branch.as_deref().unwrap_or("(detached)")
            );
            println!(
                "own commits since {}: {}",
                &o.session.base_revision[..8],
                o.commits.len()
            );
            for c in &o.commits {
                println!("  {} {}", &c.hash[..8], c.subject);
            }
            if o.changes.is_empty() {
                println!("no uncommitted changes");
            } else {
                println!("uncommitted changes:");
                for c in &o.changes {
                    println!("  {} {}", c.code, c.path);
                }
            }
        }
        Command::Session(SessionCmd::List) => {
            let ss = app.list_sessions().await?;
            if ctx.json {
                return print_json(&ss);
            }
            for s in ss {
                println!("{}", session_line(&s));
            }
        }
        Command::Session(SessionCmd::Finish { sel, abandon }) => {
            let s = ctx.session(&sel).await?;
            let outcome = if abandon {
                SessionOutcome::Abandoned
            } else {
                SessionOutcome::Completed
            };
            let s = app.finish_session(&s.id, outcome).await?;
            if ctx.json {
                return print_json(&s);
            }
            println!("session {} {}", s.id.short(), s.status.as_str());
        }

        Command::Mentor(MentorCmd::Ask {
            question,
            level,
            excerpt,
            sel,
        }) => {
            let s = ctx.session(&sel).await?;
            let level = HelpLevel::try_from(level).map_err(|e| anyhow!(e))?;
            let a = app
                .mentor_ask(
                    &s.id,
                    MentorAsk {
                        question,
                        excerpt,
                        level: Some(level),
                    },
                )
                .await?;
            if ctx.json {
                return print_json(&a);
            }
            let kind = serde_json::to_value(&a.decision.kind)?;
            println!("question type: {}", kind["type"].as_str().unwrap_or("?"));
            println!(
                "help level:    {} ({}){}",
                a.decision.help.granted.as_u8(),
                a.decision.help.granted.label(),
                a.decision
                    .help
                    .limited_by
                    .as_deref()
                    .map(|r| format!(" — {r}"))
                    .unwrap_or_default()
            );
            println!("{}", a.note);
            for l in &a.links {
                println!("  → {}  ({})", l.target, l.title);
            }
        }
        Command::Mentor(MentorCmd::History(sel)) => {
            let s = ctx.session(&sel).await?;
            let h = app.mentor_history(&s.id).await?;
            if ctx.json {
                return print_json(&h);
            }
            for e in h {
                println!(
                    "{}  level {}→{}  {}  {}",
                    e.event.at.format("%Y-%m-%d %H:%M"),
                    e.event.payload["requested_level"],
                    e.event.payload["granted_level"],
                    e.event.payload["action"].as_str().unwrap_or(""),
                    e.event.payload["question"].as_str().unwrap_or("")
                );
            }
        }
    }
    Ok(())
}
