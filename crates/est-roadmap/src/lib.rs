//! Roadmap Management for Craft Yourself itself (not a product feature).
//!
//! The roadmap is data: `roadmap/roadmap.toml` declares the roadmaps (their
//! prefixes, files and views) and every roadmap file holds `[[item]]`
//! tables. That data is the single source of truth for ids, statuses and
//! links; the Markdown under `docs/roadmaps/` and the summary block in
//! `ROADMAP.md` are generated from it and never edited by hand.
//!
//! - [`load`] / [`from_sources`] read the data;
//! - [`validate`] checks its integrity (rules `R01`–`R14`, see
//!   `docs/roadmap-management.md`);
//! - [`metrics`] computes progress from the items' own data;
//! - [`render`] produces the views.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub mod metrics;
pub mod render;
pub mod validate;

pub use validate::{Finding, Severity, validate};

/// Path of the configuration, relative to the repository root.
pub const CONFIG_PATH: &str = "roadmap/roadmap.toml";

/// `roadmap/roadmap.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Format version of the roadmap data.
    pub version: u32,
    /// Owner of items that do not name one.
    #[serde(default)]
    pub default_owner: Option<String>,
    /// The project-wide Definition of Done, shown in every view. An item's
    /// own `done_when` adds to it.
    #[serde(default)]
    pub definition_of_done: Vec<String>,
    /// Directory of the generated views, relative to the root.
    pub views_dir: String,
    /// File with the generated summary block (the entry point).
    pub summary_file: String,
    #[serde(rename = "roadmap")]
    pub roadmaps: Vec<RoadmapDef>,
}

/// One roadmap: a direction of development with its own file and view.
/// A new roadmap type is a new entry here and a new file — no code change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoadmapDef {
    /// Short id, also the name of the view (`<views_dir>/<id>.md`).
    pub id: String,
    pub title: String,
    /// Prefix of the ids created in this roadmap (`PRD`, `TEC`, …). An item
    /// keeps its id when it moves to another roadmap.
    pub prefix: String,
    /// Data file, relative to `roadmap/`.
    pub file: String,
    pub description: String,
    /// Research roadmaps: features and tasks state a hypothesis with
    /// acceptance and rejection criteria instead of acceptance criteria.
    #[serde(default)]
    pub hypothesis_driven: bool,
}

/// Level of an item in the hierarchy `Program → Epic → Feature → Task →
/// Subtask`; releases stand outside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Program,
    Epic,
    Feature,
    Task,
    Subtask,
    Release,
}

impl Kind {
    /// Depth in the hierarchy; `None` for releases.
    pub fn level(self) -> Option<u8> {
        match self {
            Kind::Program => Some(0),
            Kind::Epic => Some(1),
            Kind::Feature => Some(2),
            Kind::Task => Some(3),
            Kind::Subtask => Some(4),
            Kind::Release => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Program => "Program",
            Kind::Epic => "Epic",
            Kind::Feature => "Feature",
            Kind::Task => "Task",
            Kind::Subtask => "Subtask",
            Kind::Release => "Release",
        }
    }

    /// Implementable work: acceptance criteria are required once it is ready.
    pub fn is_work(self) -> bool {
        matches!(self, Kind::Feature | Kind::Task | Kind::Subtask)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Proposed,
    Planned,
    Ready,
    InProgress,
    InReview,
    Blocked,
    Done,
    Deferred,
    Cancelled,
}

impl Status {
    pub const ALL: [Status; 9] = [
        Status::Proposed,
        Status::Planned,
        Status::Ready,
        Status::InProgress,
        Status::InReview,
        Status::Blocked,
        Status::Done,
        Status::Deferred,
        Status::Cancelled,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Status::Proposed => "Proposed",
            Status::Planned => "Planned",
            Status::Ready => "Ready",
            Status::InProgress => "In Progress",
            Status::InReview => "In Review",
            Status::Blocked => "Blocked",
            Status::Done => "Done",
            Status::Deferred => "Deferred",
            Status::Cancelled => "Cancelled",
        }
    }

    /// Work has started (or finished): acceptance criteria must exist.
    pub fn is_committed(self) -> bool {
        matches!(
            self,
            Status::Ready | Status::InProgress | Status::InReview | Status::Blocked | Status::Done
        )
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Status::InProgress | Status::InReview | Status::Blocked
        )
    }

    /// No more work is expected under this status.
    pub fn is_closed(self) -> bool {
        matches!(self, Status::Done | Status::Cancelled)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Risk {
    Low,
    Medium,
    High,
}

/// Link types between items (§5.8). `parent` is the hierarchy; `release`
/// targets a release item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    DependsOn,
    Blocks,
    RelatedTo,
    Implements,
    Validates,
    Extends,
    Replaces,
}

impl LinkType {
    pub const ALL: [LinkType; 7] = [
        LinkType::DependsOn,
        LinkType::Blocks,
        LinkType::RelatedTo,
        LinkType::Implements,
        LinkType::Validates,
        LinkType::Extends,
        LinkType::Replaces,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LinkType::DependsOn => "Depends on",
            LinkType::Blocks => "Blocks",
            LinkType::RelatedTo => "Related to",
            LinkType::Implements => "Implements",
            LinkType::Validates => "Validates",
            LinkType::Extends => "Extends",
            LinkType::Replaces => "Replaces",
        }
    }
}

/// A recorded change of a large item (§5.11). Git keeps the full history;
/// this is the reasoning that a diff does not show.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub summary: String,
    pub reason: String,
    #[serde(default)]
    pub impact: Option<String>,
    /// An ADR or other document describing the decision.
    #[serde(default)]
    pub decision: Option<String>,
}

/// One element of a roadmap (§5.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub status: Status,
    #[serde(default)]
    pub priority: Option<Priority>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub parent: Option<String>,

    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub related_to: Vec<String>,
    #[serde(default)]
    pub implements: Vec<String>,
    #[serde(default)]
    pub validates: Vec<String>,
    #[serde(default)]
    pub extends: Vec<String>,
    #[serde(default)]
    pub replaces: Vec<String>,

    /// Checklist: each entry starts with `[ ] ` or `[x] `.
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Item-specific Definition of Done, same checklist format.
    #[serde(default)]
    pub done_when: Vec<String>,

    /// Research items: the hypothesis and how it is accepted or rejected.
    #[serde(default)]
    pub hypothesis: Option<String>,
    #[serde(default)]
    pub accept_if: Option<String>,
    #[serde(default)]
    pub reject_if: Option<String>,
    /// Outcome of a finished experiment.
    #[serde(default)]
    pub result: Option<String>,

    #[serde(default)]
    pub estimate_days: Option<f64>,
    #[serde(default)]
    pub actual_days: Option<f64>,
    #[serde(default)]
    pub risk: Option<Risk>,
    /// Target release (a `release` item).
    #[serde(default)]
    pub release: Option<String>,
    /// Due date, `YYYY-MM-DD`.
    #[serde(default)]
    pub due: Option<String>,
    /// Release items: the version number.
    #[serde(default)]
    pub version: Option<String>,

    /// Repository paths: directories, crates, migrations, routes, tests.
    /// Their existence is checked, but never counts as completion.
    #[serde(default)]
    pub components: Vec<String>,
    /// Related documentation (paths, optionally with `#anchor`).
    #[serde(default)]
    pub docs: Vec<String>,
    /// Free tags; `debt` marks technical debt.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Where the requirement comes from (`TZ 2.1`, `spec v0.3 §11`, …).
    #[serde(default)]
    pub source: Option<String>,
    /// Why the item is blocked, deferred or cancelled (required for those
    /// statuses unless a dependency explains a block), or why it lags behind
    /// a linked item.
    #[serde(default)]
    pub status_reason: Option<String>,
    #[serde(default)]
    pub changes: Vec<Change>,
}

impl Item {
    /// Explicit links of one type.
    pub fn links(&self, t: LinkType) -> &[String] {
        match t {
            LinkType::DependsOn => &self.depends_on,
            LinkType::Blocks => &self.blocks,
            LinkType::RelatedTo => &self.related_to,
            LinkType::Implements => &self.implements,
            LinkType::Validates => &self.validates,
            LinkType::Extends => &self.extends,
            LinkType::Replaces => &self.replaces,
        }
    }

    pub fn is_debt(&self) -> bool {
        self.tags.iter().any(|t| t == "debt")
    }
}

/// A checklist entry: `[x] text` → `(true, "text")`.
pub fn criterion(entry: &str) -> Option<(bool, &str)> {
    if let Some(t) = entry.strip_prefix("[x] ") {
        Some((true, t))
    } else {
        entry.strip_prefix("[ ] ").map(|t| (false, t))
    }
}

/// An item with where it was read from.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    #[serde(flatten)]
    pub item: Item,
    /// Id of the roadmap whose file holds the item.
    pub roadmap: String,
}

/// The whole roadmap: configuration and every item, in file order.
#[derive(Debug, Clone, Serialize)]
pub struct Roadmap {
    pub config: Config,
    pub items: Vec<Entry>,
}

impl Roadmap {
    /// Index of items by id (the first one when ids repeat).
    pub fn index(&self) -> BTreeMap<&str, &Entry> {
        let mut m = BTreeMap::new();
        for e in &self.items {
            m.entry(e.item.id.as_str()).or_insert(e);
        }
        m
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.items.iter().find(|e| e.item.id == id)
    }

    pub fn roadmap(&self, id: &str) -> Option<&RoadmapDef> {
        self.config.roadmaps.iter().find(|r| r.id == id)
    }

    /// Roadmap of an id's prefix (where the id was created).
    pub fn roadmap_of_prefix(&self, id: &str) -> Option<&RoadmapDef> {
        let prefix = id.split('-').next()?;
        self.config.roadmaps.iter().find(|r| r.prefix == prefix)
    }

    /// Children of an item, in file order.
    pub fn children<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Entry> + 'a {
        self.items
            .iter()
            .filter(move |e| e.item.parent.as_deref() == Some(id))
    }

    /// Effective dependencies of an item: its `depends_on` plus every item
    /// that declares `blocks` on it.
    pub fn dependencies(&self, id: &str) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        if let Some(e) = self.get(id) {
            out.extend(e.item.depends_on.iter().map(String::as_str));
        }
        for e in &self.items {
            if e.item.blocks.iter().any(|b| b == id) && !out.contains(&e.item.id.as_str()) {
                out.push(e.item.id.as_str());
            }
        }
        out
    }

    /// Releases in the order they are declared.
    pub fn releases(&self) -> impl Iterator<Item = &Entry> {
        self.items.iter().filter(|e| e.item.kind == Kind::Release)
    }

    /// Position of a release in [`Roadmap::releases`].
    pub fn release_order(&self, id: &str) -> Option<usize> {
        self.releases().position(|e| e.item.id == id)
    }

    pub fn owner<'a>(&'a self, item: &'a Item) -> Option<&'a str> {
        item.owner
            .as_deref()
            .or(self.config.default_owner.as_deref())
    }
}

#[derive(Debug)]
pub enum LoadError {
    Io(String, std::io::Error),
    Parse(String, String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(p, e) => write!(f, "{p}: {e}"),
            LoadError::Parse(p, e) => write!(f, "{p}: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    item: Vec<Item>,
}

/// Parses the configuration and the roadmap files given as
/// `(file name relative to roadmap/, contents)`. A configured file that is
/// not given is treated as empty.
pub fn from_sources(config: &str, files: &[(&str, &str)]) -> Result<Roadmap, LoadError> {
    let config: Config =
        toml::from_str(config).map_err(|e| LoadError::Parse(CONFIG_PATH.into(), e.to_string()))?;
    let mut items = Vec::new();
    for def in &config.roadmaps {
        let Some((_, text)) = files.iter().find(|(f, _)| *f == def.file) else {
            continue;
        };
        let file: File = toml::from_str(text)
            .map_err(|e| LoadError::Parse(format!("roadmap/{}", def.file), e.to_string()))?;
        items.extend(file.item.into_iter().map(|item| Entry {
            item,
            roadmap: def.id.clone(),
        }));
    }
    Ok(Roadmap { config, items })
}

/// Reads `roadmap/` under the repository root.
pub fn load(root: &Path) -> Result<Roadmap, LoadError> {
    let read = |rel: &str| {
        let p = root.join(rel);
        std::fs::read_to_string(&p).map_err(|e| LoadError::Io(rel.to_string(), e))
    };
    let config = read(CONFIG_PATH)?;
    let parsed: Config =
        toml::from_str(&config).map_err(|e| LoadError::Parse(CONFIG_PATH.into(), e.to_string()))?;
    let mut texts = Vec::new();
    for def in &parsed.roadmaps {
        texts.push((def.file.clone(), read(&format!("roadmap/{}", def.file))?));
    }
    let files: Vec<(&str, &str)> = texts
        .iter()
        .map(|(f, t)| (f.as_str(), t.as_str()))
        .collect();
    from_sources(&config, &files)
}

/// Finds the repository root: the closest ancestor with `roadmap/roadmap.toml`.
pub fn find_root(start: &Path) -> Option<std::path::PathBuf> {
    start
        .ancestors()
        .find(|p| p.join(CONFIG_PATH).is_file())
        .map(Path::to_path_buf)
}

/// Lowercase anchor of an id (`PRD-001` → `prd-001`).
pub fn anchor(id: &str) -> String {
    id.to_ascii_lowercase()
}
