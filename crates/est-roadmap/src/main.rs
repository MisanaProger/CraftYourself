//! `est-roadmap` — checks the roadmap data and renders its views.
//!
//! ```sh
//! cargo run -p est-roadmap -- check            # integrity rules; fails on errors
//! cargo run -p est-roadmap -- render           # rewrite docs/roadmaps/* and the ROADMAP.md summary
//! cargo run -p est-roadmap -- render --check   # fail if the views are out of date
//! cargo run -p est-roadmap -- metrics          # progress, blocked, overdue (today)
//! cargo run -p est-roadmap -- graph --format dot
//! cargo run -p est-roadmap -- export           # everything as JSON
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use est_roadmap::{Roadmap, Severity, find_root, load, metrics, render, validate};

#[derive(Parser)]
#[command(name = "est-roadmap", about = "Craft Yourself roadmap management")]
struct Cli {
    /// Repository root (default: the closest ancestor with roadmap/roadmap.toml).
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check integrity; exit code 1 on errors (warnings are printed).
    Check {
        /// Treat warnings as errors.
        #[arg(long)]
        strict: bool,
    },
    /// Write the generated views and the summary block.
    Render {
        /// Do not write; fail if anything would change.
        #[arg(long)]
        check: bool,
    },
    /// Print progress, blocked and overdue items.
    Metrics {
        /// Date for overdue items, YYYY-MM-DD (default: today).
        #[arg(long)]
        today: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Print the dependency graph.
    Graph {
        #[arg(long, value_enum, default_value_t = Format::Mermaid)]
        format: Format,
        /// Include edges between closed items.
        #[arg(long)]
        all: bool,
    },
    /// Print the configuration, items and findings as JSON.
    Export,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Mermaid,
    Dot,
}

fn exists(root: &Path) -> impl Fn(&str) -> bool + '_ {
    move |p: &str| root.join(p).exists()
}

/// Every generated file with its new contents, the summary file included.
fn rendered(root: &Path, rm: &Roadmap) -> Result<Vec<(PathBuf, String)>> {
    let mut files: Vec<(PathBuf, String)> = render::views(rm)
        .into_iter()
        .map(|(p, c)| (root.join(p), c))
        .collect();
    let summary = root.join(&rm.config.summary_file);
    let current = std::fs::read_to_string(&summary)
        .with_context(|| format!("reading {}", summary.display()))?;
    let next = render::splice_summary(&current, &render::summary_block(rm)).ok_or_else(|| {
        anyhow!(
            "{} has no summary markers ({} … {})",
            rm.config.summary_file,
            render::SUMMARY_BEGIN,
            render::SUMMARY_END
        )
    })?;
    files.push((summary, next));
    Ok(files)
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let root = match cli.root {
        Some(r) => r,
        None => find_root(&std::env::current_dir()?)
            .ok_or_else(|| anyhow!("no roadmap/roadmap.toml here or above"))?,
    };
    let rm = load(&root)?;
    match cli.command {
        Command::Check { strict } => {
            let findings = validate(&rm, &exists(&root));
            for f in &findings {
                eprintln!("{f}");
            }
            let errors = findings
                .iter()
                .filter(|f| f.severity == Severity::Error)
                .count();
            let warnings = findings.len() - errors;
            eprintln!(
                "{} items in {} roadmaps: {errors} errors, {warnings} warnings",
                rm.items.len(),
                rm.config.roadmaps.len()
            );
            Ok(if errors > 0 || (strict && warnings > 0) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::Render { check } => {
            // Missing paths (R11) do not stop rendering: the views may be
            // among them. `check` still reports them.
            let errors: Vec<_> = validate(&rm, &exists(&root))
                .into_iter()
                .filter(|f| f.severity == Severity::Error && f.rule != "R11")
                .collect();
            if !errors.is_empty() {
                for f in &errors {
                    eprintln!("{f}");
                }
                eprintln!("fix the errors before rendering");
                return Ok(ExitCode::FAILURE);
            }
            let mut stale = Vec::new();
            for (path, contents) in rendered(&root, &rm)? {
                let old = std::fs::read_to_string(&path).unwrap_or_default();
                if old == contents {
                    continue;
                }
                stale.push(path.clone());
                if !check {
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir)?;
                    }
                    std::fs::write(&path, contents)
                        .with_context(|| format!("writing {}", path.display()))?;
                }
            }
            for p in &stale {
                let rel = p.strip_prefix(&root).unwrap_or(p);
                eprintln!(
                    "{} {}",
                    if check { "out of date:" } else { "wrote" },
                    rel.display()
                );
            }
            if check && !stale.is_empty() {
                eprintln!("run `cargo run -p est-roadmap -- render`");
                return Ok(ExitCode::FAILURE);
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Metrics { today, json } => {
            let today = today.unwrap_or_else(|| {
                chrono::Local::now()
                    .date_naive()
                    .format("%Y-%m-%d")
                    .to_string()
            });
            let m = metrics::metrics(&rm);
            let overdue: Vec<&str> = metrics::overdue(&rm, &today)
                .iter()
                .map(|e| e.item.id.as_str())
                .collect();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "today": today, "metrics": m, "overdue": overdue
                    }))?
                );
                return Ok(ExitCode::SUCCESS);
            }
            println!("{} items", m.items);
            for (s, n) in &m.by_status {
                println!("  {:<12} {n}", s.label());
            }
            println!("\nprograms:");
            for (id, p) in &m.programs {
                let name = rm.get(id).map(|e| e.item.name.as_str()).unwrap_or_default();
                println!(
                    "  {id:<8} {:>5}  {name}",
                    p.percent().map(|v| format!("{v}%")).unwrap_or("—".into())
                );
            }
            println!("\nreleases:");
            for r in &m.releases {
                println!(
                    "  {:<8} {:<8} {:<12} {:>5}  {}",
                    r.id,
                    r.version.clone().unwrap_or_default(),
                    r.status.label(),
                    r.progress
                        .percent()
                        .map(|v| format!("{v}%"))
                        .unwrap_or("—".into()),
                    r.name
                );
            }
            println!(
                "\nacceptance criteria met: {}/{}",
                m.criteria_met, m.criteria_total
            );
            let list = |v: &[&str]| {
                if v.is_empty() {
                    "none".to_string()
                } else {
                    v.join(", ")
                }
            };
            let blocked: Vec<&str> = m.blocked.iter().map(String::as_str).collect();
            println!("blocked: {}", list(&blocked));
            println!(
                "technical debt: {} items, {} days",
                m.debt_open.len(),
                m.debt_days
            );
            println!("overdue on {today}: {}", list(&overdue));
            Ok(ExitCode::SUCCESS)
        }
        Command::Graph { format, all } => {
            print!(
                "{}",
                match format {
                    Format::Mermaid => render::mermaid(&rm, all),
                    Format::Dot => render::dot(&rm, all),
                }
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Export => {
            let findings = validate(&rm, &exists(&root));
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "format": "craft-yourself-roadmap/1",
                    "config": rm.config,
                    "items": rm.items,
                    "findings": findings,
                }))?
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
