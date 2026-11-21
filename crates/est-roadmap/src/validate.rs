//! Integrity rules of the roadmap (§5.12). Each finding names its rule so
//! `docs/roadmap-management.md` can explain it.
//!
//! | Rule | Checks |
//! |---|---|
//! | R01 | id format `XXX-NNN` with a configured prefix |
//! | R02 | ids are unique |
//! | R03 | required fields per kind |
//! | R04 | parent exists, sits higher in the hierarchy, no parent loops |
//! | R05 | links and `release` point at existing items of the right kind |
//! | R06 | no dependency cycles (`depends_on` and inverted `blocks`) |
//! | R07 | checklist format; acceptance criteria (or a hypothesis) for committed work |
//! | R08 | `done` only with every criterion and `done_when` entry met |
//! | R09 | statuses agree with parents, children and dependencies |
//! | R10 | release planning: nothing depends on later or cancelled work |
//! | R11 | listed paths exist (required for done items) |
//! | R12 | dates and numbers are well-formed |
//! | R13 | a release is done only when its items are closed |
//! | R14 | linked views agree: an implemented item is not left planned |

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::{Kind, LinkType, Roadmap, Status, criterion};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub rule: &'static str,
    /// The item the finding is about, if any.
    pub id: Option<String>,
    pub message: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        match &self.id {
            Some(id) => write!(f, "{sev}[{}] {id}: {}", self.rule, self.message),
            None => write!(f, "{sev}[{}]: {}", self.rule, self.message),
        }
    }
}

struct Out(Vec<Finding>);

impl Out {
    fn err(&mut self, rule: &'static str, id: &str, msg: impl Into<String>) {
        self.0.push(Finding {
            severity: Severity::Error,
            rule,
            id: Some(id.to_string()),
            message: msg.into(),
        });
    }
    fn warn(&mut self, rule: &'static str, id: &str, msg: impl Into<String>) {
        self.0.push(Finding {
            severity: Severity::Warning,
            rule,
            id: Some(id.to_string()),
            message: msg.into(),
        });
    }
}

fn valid_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok() && s.len() == 10
}

/// Checks the roadmap. `path_exists` answers whether a repository path
/// (relative to the root, without `#anchor`) exists.
pub fn validate(rm: &Roadmap, path_exists: &dyn Fn(&str) -> bool) -> Vec<Finding> {
    let mut out = Out(Vec::new());
    let prefixes: BTreeSet<&str> = rm
        .config
        .roadmaps
        .iter()
        .map(|r| r.prefix.as_str())
        .collect();
    let mut seen_prefix_ids: BTreeSet<&str> = BTreeSet::new();
    for r in &rm.config.roadmaps {
        if !seen_prefix_ids.insert(r.prefix.as_str()) {
            out.0.push(Finding {
                severity: Severity::Error,
                rule: "R01",
                id: None,
                message: format!("prefix {} is used by more than one roadmap", r.prefix),
            });
        }
    }
    for entry in rm.config.definition_of_done.iter() {
        if entry.trim().is_empty() {
            out.0.push(Finding {
                severity: Severity::Error,
                rule: "R03",
                id: None,
                message: "empty entry in definition_of_done".into(),
            });
        }
    }

    let index = rm.index();
    let status = |id: &str| index.get(id).map(|e| e.item.status);

    // R01, R02
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for e in &rm.items {
        *counts.entry(e.item.id.as_str()).or_default() += 1;
    }
    for (id, n) in &counts {
        if *n > 1 {
            out.err("R02", id, format!("id is used by {n} items"));
        }
    }
    for e in &rm.items {
        let it = &e.item;
        let ok = it.id.split_once('-').is_some_and(|(p, n)| {
            prefixes.contains(p) && n.len() >= 3 && n.chars().all(|c| c.is_ascii_digit())
        });
        if !ok {
            out.err(
                "R01",
                &it.id,
                format!(
                    "id must be PREFIX-NNN with a configured prefix ({})",
                    prefixes.iter().copied().collect::<Vec<_>>().join(", ")
                ),
            );
        }
        if it.kind == Kind::Release && !it.id.starts_with("REL-") && prefixes.contains("REL") {
            out.warn("R01", &it.id, "releases normally use the REL prefix");
        }
    }

    for e in &rm.items {
        let it = &e.item;
        let id = it.id.as_str();
        let def = rm.roadmap(&e.roadmap);
        let hypothesis_driven = def.is_some_and(|d| d.hypothesis_driven);

        // R03 required fields
        if it.name.trim().is_empty() {
            out.err("R03", id, "name is empty");
        }
        if it.kind != Kind::Release && it.priority.is_none() {
            out.err("R03", id, "priority is required");
        }
        if matches!(
            it.kind,
            Kind::Program | Kind::Epic | Kind::Feature | Kind::Release
        ) && it
            .description
            .as_deref()
            .is_none_or(|d| d.trim().is_empty())
        {
            out.err(
                "R03",
                id,
                format!("{} needs a description", it.kind.label()),
            );
        }
        if it.kind == Kind::Release && it.version.is_none() {
            out.err("R03", id, "release needs a version");
        }
        if matches!(
            it.status,
            Status::Blocked | Status::Deferred | Status::Cancelled
        ) && it.status_reason.is_none()
        {
            let explained = it.status == Status::Blocked
                && rm
                    .dependencies(id)
                    .iter()
                    .any(|d| status(d).is_some_and(|s| !s.is_closed()));
            if !explained {
                out.err(
                    "R03",
                    id,
                    format!(
                        "status {} needs status_reason{}",
                        it.status,
                        if it.status == Status::Blocked {
                            " or an open dependency"
                        } else {
                            ""
                        }
                    ),
                );
            }
        }

        // R04 parent
        match (&it.parent, it.kind) {
            (None, Kind::Program | Kind::Release) => {}
            (None, k) => out.err("R04", id, format!("{} needs a parent", k.label())),
            (Some(_), Kind::Program | Kind::Release) => {
                out.err("R04", id, format!("a {} has no parent", it.kind.label()));
            }
            (Some(p), k) => match index.get(p.as_str()) {
                None => out.err("R04", id, format!("parent {p} does not exist")),
                Some(pe) => {
                    let ok = matches!(
                        (pe.item.kind.level(), k.level()),
                        (Some(a), Some(b)) if a < b
                    );
                    if !ok {
                        out.err(
                            "R04",
                            id,
                            format!(
                                "parent {p} is a {}, it must be higher than a {}",
                                pe.item.kind.label(),
                                k.label()
                            ),
                        );
                    }
                }
            },
        }

        // R05 links
        for t in LinkType::ALL {
            let mut uniq = BTreeSet::new();
            for target in it.links(t) {
                if target == id {
                    out.err("R05", id, format!("{} links to itself", t.label()));
                } else if !index.contains_key(target.as_str()) {
                    out.err("R05", id, format!("{} {target}: no such item", t.label()));
                } else if index[target.as_str()].item.kind == Kind::Release {
                    out.err(
                        "R05",
                        id,
                        format!(
                            "{} {target}: link to a release, use `release` instead",
                            t.label()
                        ),
                    );
                }
                if !uniq.insert(target) {
                    out.warn("R05", id, format!("{} {target} is listed twice", t.label()));
                }
            }
        }
        if let Some(r) = &it.release {
            if it.kind == Kind::Release {
                out.err("R05", id, "a release cannot target a release");
            } else {
                match index.get(r.as_str()) {
                    Some(re) if re.item.kind == Kind::Release => {}
                    Some(_) => out.err("R05", id, format!("release {r} is not a release")),
                    None => out.err("R05", id, format!("release {r} does not exist")),
                }
            }
        }

        // R07 checklists and acceptance criteria
        for (field, list) in [("acceptance", &it.acceptance), ("done_when", &it.done_when)] {
            for c in list {
                if criterion(c).is_none() {
                    out.err(
                        "R07",
                        id,
                        format!("{field} entry must start with \"[ ] \" or \"[x] \": {c:?}"),
                    );
                }
            }
        }
        if it.kind.is_work() && it.status.is_committed() {
            if hypothesis_driven {
                if it.hypothesis.is_none() || it.accept_if.is_none() || it.reject_if.is_none() {
                    out.err(
                        "R07",
                        id,
                        "research work needs hypothesis, accept_if and reject_if",
                    );
                }
            } else if it.acceptance.is_empty() {
                out.err(
                    "R07",
                    id,
                    format!(
                        "{} {} without acceptance criteria",
                        it.kind.label(),
                        it.status
                    ),
                );
            }
        }

        // R08 done
        if it.status == Status::Done {
            let open: Vec<&str> = it
                .acceptance
                .iter()
                .chain(&it.done_when)
                .filter_map(|c| criterion(c))
                .filter(|(met, _)| !met)
                .map(|(_, t)| t)
                .collect();
            if !open.is_empty() {
                out.err(
                    "R08",
                    id,
                    format!("done with unmet criteria: {}", open.join("; ")),
                );
            }
            if hypothesis_driven && it.kind.is_work() && it.result.is_none() {
                out.err("R08", id, "a finished experiment needs a result");
            }
        }

        // R09 status consistency
        if let Some(p) = it.parent.as_deref().and_then(|p| index.get(p)) {
            let ps = p.item.status;
            if ps == Status::Done
                && !matches!(
                    it.status,
                    Status::Done | Status::Cancelled | Status::Deferred
                )
            {
                out.err(
                    "R09",
                    id,
                    format!("parent {} is Done while this is {}", p.item.id, it.status),
                );
            }
            if ps == Status::Cancelled && !matches!(it.status, Status::Cancelled | Status::Done) {
                out.err(
                    "R09",
                    id,
                    format!(
                        "parent {} is Cancelled while this is {}",
                        p.item.id, it.status
                    ),
                );
            }
            if matches!(ps, Status::Proposed | Status::Planned | Status::Deferred)
                && (it.status.is_active() || it.status == Status::Done)
            {
                out.warn(
                    "R09",
                    id,
                    format!(
                        "this is {} but parent {} is still {}",
                        it.status, p.item.id, ps
                    ),
                );
            }
        }
        for dep in rm.dependencies(id) {
            let Some(ds) = status(dep) else { continue };
            if it.status == Status::Done && ds != Status::Done {
                out.err("R09", id, format!("Done, but its dependency {dep} is {ds}"));
            } else if matches!(
                it.status,
                Status::Ready | Status::InProgress | Status::InReview
            ) && !ds.is_closed()
            {
                out.warn(
                    "R09",
                    id,
                    format!("{} while dependency {dep} is {ds}", it.status),
                );
            }
            // R10
            if ds == Status::Cancelled && !it.status.is_closed() {
                out.warn("R10", id, format!("depends on cancelled {dep}"));
            }
            if let (Some(mine), Some(theirs)) = (
                it.release.as_deref().and_then(|r| rm.release_order(r)),
                index
                    .get(dep)
                    .and_then(|d| d.item.release.as_deref())
                    .and_then(|r| rm.release_order(r)),
            ) && theirs > mine
                && ds != Status::Done
            {
                out.err(
                    "R10",
                    id,
                    format!(
                        "targets {} but depends on {dep}, planned for the later {}",
                        it.release.as_deref().unwrap_or_default(),
                        index[dep].item.release.as_deref().unwrap_or_default()
                    ),
                );
            }
        }
        for r in &it.replaces {
            if let Some(s) = status(r)
                && !s.is_closed()
                && it.status == Status::Done
            {
                out.warn("R10", id, format!("replaces {r}, which is still {s}"));
            }
        }

        // R11 paths
        for (field, list) in [("components", &it.components), ("docs", &it.docs)] {
            for p in list {
                let path = p.split('#').next().unwrap_or(p);
                if path.is_empty() || path.starts_with('/') || path.contains("..") {
                    out.err("R11", id, format!("{field}: {p:?} must be a relative path"));
                } else if !path_exists(path) && it.status == Status::Done {
                    out.err(
                        "R11",
                        id,
                        format!("{field}: {path} does not exist, but the item is Done"),
                    );
                }
            }
        }

        // R12 well-formed values
        if let Some(d) = &it.due
            && !valid_date(d)
        {
            out.err("R12", id, format!("due {d:?} is not YYYY-MM-DD"));
        }
        for c in &it.changes {
            if !valid_date(&c.date) {
                out.err(
                    "R12",
                    id,
                    format!("change date {:?} is not YYYY-MM-DD", c.date),
                );
            }
        }
        for (name, v) in [
            ("estimate_days", it.estimate_days),
            ("actual_days", it.actual_days),
        ] {
            if v.is_some_and(|v| !v.is_finite() || v < 0.0) {
                out.err("R12", id, format!("{name} must be a non-negative number"));
            }
        }

        // R13 releases
        if it.kind == Kind::Release && it.status == Status::Done {
            for other in &rm.items {
                if other.item.release.as_deref() == Some(id) && !other.item.status.is_closed() {
                    out.err(
                        "R13",
                        id,
                        format!("Done, but {} is {}", other.item.id, other.item.status),
                    );
                }
            }
        }
        if let Some(r) = it.release.as_deref().and_then(|r| index.get(r))
            && r.item.status == Status::Done
            && !it.status.is_closed()
        {
            out.err(
                "R13",
                id,
                format!("targets {}, which is already Done", r.item.id),
            );
        }

        // R14 cross-roadmap consistency
        for target in &it.implements {
            let Some(t) = index.get(target.as_str()) else {
                continue;
            };
            let ts = t.item.status;
            if it.status == Status::Done
                && matches!(ts, Status::Proposed | Status::Planned | Status::Ready)
                && t.item.status_reason.is_none()
            {
                out.err(
                    "R14",
                    target,
                    format!(
                        "is {ts} although {id}, which implements it, is Done; \
                         update it or explain in status_reason"
                    ),
                );
            }
        }
    }
    // R14: an implemented item marked Done while an implementation is open.
    for e in &rm.items {
        if e.item.status != Status::Done {
            continue;
        }
        for other in &rm.items {
            if other.item.implements.contains(&e.item.id)
                && !other.item.status.is_closed()
                && other.item.status != Status::Deferred
            {
                out.err(
                    "R14",
                    &e.item.id,
                    format!(
                        "Done, but {}, which implements it, is {}",
                        other.item.id, other.item.status
                    ),
                );
            }
        }
    }

    // R04 parent loops
    for e in &rm.items {
        let mut seen = BTreeSet::new();
        let mut cur = e.item.id.as_str();
        while let Some(p) = index.get(cur).and_then(|x| x.item.parent.as_deref()) {
            if !seen.insert(cur) {
                out.err("R04", &e.item.id, "parent chain loops");
                break;
            }
            cur = p;
        }
    }

    // R06 cycles
    for cycle in dependency_cycles(rm) {
        let first = cycle[0].clone();
        out.err(
            "R06",
            &first,
            format!("dependency cycle: {} → {first}", cycle.join(" → ")),
        );
    }

    let mut v = out.0;
    v.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.rule.cmp(b.rule))
    });
    v.dedup();
    v
}

/// Every elementary dependency cycle found by a depth-first search, each
/// rotated to start at its smallest id and reported once.
pub fn dependency_cycles(rm: &Roadmap) -> Vec<Vec<String>> {
    let ids: Vec<&str> = rm.items.iter().map(|e| e.item.id.as_str()).collect();
    let edges: BTreeMap<&str, Vec<&str>> =
        ids.iter().map(|id| (*id, rm.dependencies(id))).collect();
    let mut found: BTreeSet<Vec<String>> = BTreeSet::new();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    fn dfs<'a>(
        n: &'a str,
        edges: &BTreeMap<&'a str, Vec<&'a str>>,
        path: &mut Vec<&'a str>,
        done: &mut BTreeSet<&'a str>,
        found: &mut BTreeSet<Vec<String>>,
    ) {
        if let Some(pos) = path.iter().position(|p| *p == n) {
            let mut c: Vec<String> = path[pos..].iter().map(|s| s.to_string()).collect();
            let min = c
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.cmp(b.1))
                .map(|(i, _)| i)
                .unwrap_or(0);
            c.rotate_left(min);
            found.insert(c);
            return;
        }
        if done.contains(n) {
            return;
        }
        path.push(n);
        for m in edges.get(n).into_iter().flatten() {
            dfs(m, edges, path, done, found);
        }
        path.pop();
        done.insert(n);
    }
    for id in &ids {
        let mut path = Vec::new();
        dfs(id, &edges, &mut path, &mut done, &mut found);
    }
    found.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::from_sources;

    const CONFIG: &str = r#"
version = 1
views_dir = "docs/roadmaps"
summary_file = "ROADMAP.md"
[[roadmap]]
id = "product"
title = "Product"
prefix = "PRD"
file = "product.toml"
description = "d"
[[roadmap]]
id = "technical"
title = "Technical"
prefix = "TEC"
file = "technical.toml"
description = "d"
[[roadmap]]
id = "research"
title = "Research"
prefix = "RND"
file = "research.toml"
description = "d"
hypothesis_driven = true
[[roadmap]]
id = "releases"
title = "Releases"
prefix = "REL"
file = "releases.toml"
description = "d"
"#;

    fn check(product: &str, technical: &str) -> Vec<Finding> {
        check_all(product, technical, "", "")
    }

    fn check_all(product: &str, technical: &str, research: &str, releases: &str) -> Vec<Finding> {
        let rm = from_sources(
            CONFIG,
            &[
                ("product.toml", product),
                ("technical.toml", technical),
                ("research.toml", research),
                ("releases.toml", releases),
            ],
        )
        .expect("parses");
        validate(&rm, &|p| p == "crates/est-core")
    }

    fn rules(f: &[Finding], sev: Severity) -> Vec<(&'static str, String)> {
        f.iter()
            .filter(|x| x.severity == sev)
            .map(|x| (x.rule, x.id.clone().unwrap_or_default()))
            .collect()
    }

    const PROGRAM: &str = r#"
[[item]]
id = "PRD-001"
name = "Program"
kind = "program"
status = "in-progress"
priority = "P0"
description = "p"
"#;

    #[test]
    fn a_consistent_roadmap_has_no_findings() {
        let p = format!(
            "{PROGRAM}
[[item]]
id = \"PRD-002\"
name = \"Feature\"
kind = \"feature\"
status = \"done\"
priority = \"P0\"
parent = \"PRD-001\"
description = \"f\"
acceptance = [\"[x] works\"]
components = [\"crates/est-core\"]
"
        );
        assert_eq!(check(&p, ""), vec![]);
    }

    #[test]
    fn links_must_point_at_existing_items() {
        let p = format!(
            "{PROGRAM}
[[item]]
id = \"PRD-002\"
name = \"F\"
kind = \"epic\"
status = \"planned\"
priority = \"P1\"
parent = \"PRD-001\"
description = \"f\"
depends_on = [\"PRD-099\", \"PRD-002\"]
release = \"PRD-001\"
"
        );
        let f = check(&p, "");
        let r05: Vec<&str> = f
            .iter()
            .filter(|x| x.rule == "R05")
            .map(|x| x.message.as_str())
            .collect();
        assert_eq!(r05.len(), 3, "{f:?}");
        assert!(r05.iter().any(|m| m.contains("PRD-099: no such item")));
        assert!(r05.iter().any(|m| m.contains("links to itself")));
        assert!(r05.iter().any(|m| m.contains("is not a release")));
    }

    #[test]
    fn ids_must_be_well_formed_and_unique() {
        let p = format!("{PROGRAM}{}", PROGRAM.replace("PRD-001", "XYZ-1"));
        let p = format!("{p}{PROGRAM}");
        let f = check(&p, "");
        let e = rules(&f, Severity::Error);
        assert!(e.contains(&("R01", "XYZ-1".into())), "{f:?}");
        assert!(e.contains(&("R02", "PRD-001".into())), "{f:?}");
    }

    #[test]
    fn hierarchy_needs_a_higher_parent() {
        let p = format!(
            "{PROGRAM}
[[item]]
id = \"PRD-002\"
name = \"Task\"
kind = \"task\"
status = \"planned\"
priority = \"P1\"
parent = \"PRD-003\"

[[item]]
id = \"PRD-003\"
name = \"Subtask\"
kind = \"subtask\"
status = \"planned\"
priority = \"P1\"
parent = \"PRD-001\"

[[item]]
id = \"PRD-004\"
name = \"Orphan\"
kind = \"task\"
status = \"planned\"
priority = \"P1\"
"
        );
        let e = rules(&check(&p, ""), Severity::Error);
        assert!(e.contains(&("R04", "PRD-002".into())));
        assert!(e.contains(&("R04", "PRD-004".into())));
        assert!(!e.contains(&("R04", "PRD-003".into())));
    }

    #[test]
    fn detects_dependency_cycles_across_roadmaps() {
        let p = format!(
            "{PROGRAM}
[[item]]
id = \"PRD-002\"
name = \"A\"
kind = \"epic\"
status = \"planned\"
priority = \"P1\"
parent = \"PRD-001\"
description = \"a\"
depends_on = [\"TEC-002\"]
"
        );
        let t = r#"
[[item]]
id = "TEC-001"
name = "T"
kind = "program"
status = "planned"
priority = "P1"
description = "t"
[[item]]
id = "TEC-002"
name = "B"
kind = "epic"
status = "planned"
priority = "P1"
parent = "TEC-001"
description = "b"
[[item]]
id = "TEC-003"
name = "C"
kind = "epic"
status = "planned"
priority = "P1"
parent = "TEC-001"
description = "c"
blocks = ["TEC-002"]
depends_on = ["PRD-002"]
"#;
        let f = check(&p, t);
        let cycles: Vec<&Finding> = f.iter().filter(|x| x.rule == "R06").collect();
        assert_eq!(cycles.len(), 1, "{f:?}");
        assert!(
            cycles[0].message.contains("PRD-002 → TEC-002 → TEC-003"),
            "{}",
            cycles[0].message
        );
    }

    #[test]
    fn done_needs_met_criteria_and_closed_dependencies() {
        let p = format!(
            "{PROGRAM}
[[item]]
id = \"PRD-002\"
name = \"F\"
kind = \"feature\"
status = \"done\"
priority = \"P0\"
parent = \"PRD-001\"
description = \"f\"
acceptance = [\"[x] a\", \"[ ] b\"]
depends_on = [\"PRD-003\"]
components = [\"crates/missing\"]

[[item]]
id = \"PRD-003\"
name = \"G\"
kind = \"feature\"
status = \"in-progress\"
priority = \"P0\"
parent = \"PRD-001\"
description = \"g\"
"
        );
        let e = rules(&check(&p, ""), Severity::Error);
        assert!(e.contains(&("R08", "PRD-002".into())));
        assert!(e.contains(&("R09", "PRD-002".into())));
        assert!(e.contains(&("R11", "PRD-002".into())));
        // committed work without acceptance criteria
        assert!(e.contains(&("R07", "PRD-003".into())));
    }

    #[test]
    fn statuses_need_reasons() {
        let p = format!(
            "{PROGRAM}
[[item]]
id = \"PRD-002\"
name = \"F\"
kind = \"epic\"
status = \"deferred\"
priority = \"P2\"
parent = \"PRD-001\"
description = \"f\"

[[item]]
id = \"PRD-003\"
name = \"G\"
kind = \"epic\"
status = \"blocked\"
priority = \"P2\"
parent = \"PRD-001\"
description = \"g\"
depends_on = [\"PRD-002\"]
"
        );
        let e = rules(&check(&p, ""), Severity::Error);
        assert!(e.contains(&("R03", "PRD-002".into())));
        // blocked by an open dependency: explained
        assert!(!e.contains(&("R03", "PRD-003".into())));
    }

    #[test]
    fn implemented_items_cannot_stay_planned_silently() {
        let p = format!(
            "{PROGRAM}
[[item]]
id = \"PRD-002\"
name = \"User feature\"
kind = \"epic\"
status = \"planned\"
priority = \"P0\"
parent = \"PRD-001\"
description = \"f\"
"
        );
        let t = r#"
[[item]]
id = "TEC-001"
name = "T"
kind = "program"
status = "done"
priority = "P1"
description = "t"
implements = ["PRD-002"]
"#;
        let e = rules(&check(&p, t), Severity::Error);
        assert!(e.contains(&("R14", "PRD-002".into())));
        let explained = p.replace(
            "description = \"f\"",
            "description = \"f\"\nstatus_reason = \"waiting for the UI\"",
        );
        let e = rules(&check(&explained, t), Severity::Error);
        assert!(!e.contains(&("R14", "PRD-002".into())));
    }

    #[test]
    fn releases_order_dependencies() {
        let rel = r#"
[[item]]
id = "REL-001"
name = "0.1"
kind = "release"
status = "planned"
version = "0.1.0"
description = "first"
[[item]]
id = "REL-002"
name = "0.2"
kind = "release"
status = "planned"
version = "0.2.0"
description = "second"
"#;
        let p = format!(
            "{PROGRAM}
[[item]]
id = \"PRD-002\"
name = \"Early\"
kind = \"epic\"
status = \"planned\"
priority = \"P0\"
parent = \"PRD-001\"
description = \"e\"
release = \"REL-001\"
depends_on = [\"PRD-003\"]

[[item]]
id = \"PRD-003\"
name = \"Late\"
kind = \"epic\"
status = \"planned\"
priority = \"P0\"
parent = \"PRD-001\"
description = \"l\"
release = \"REL-002\"
"
        );
        let e = rules(&check_all(&p, "", "", rel), Severity::Error);
        assert!(e.contains(&("R10", "PRD-002".into())), "{e:?}");
        let done = rel.replacen("status = \"planned\"", "status = \"done\"", 1);
        let e = rules(&check_all(&p, "", "", &done), Severity::Error);
        assert!(e.contains(&("R13", "REL-001".into())), "{e:?}");
        assert!(e.contains(&("R13", "PRD-002".into())), "{e:?}");
    }

    #[test]
    fn research_needs_a_hypothesis() {
        let r = r#"
[[item]]
id = "RND-001"
name = "R"
kind = "program"
status = "in-progress"
priority = "P2"
description = "r"
[[item]]
id = "RND-002"
name = "Experiment"
kind = "feature"
status = "in-progress"
priority = "P2"
parent = "RND-001"
description = "e"
"#;
        let e = rules(&check_all("", "", r, ""), Severity::Error);
        assert!(e.contains(&("R07", "RND-002".into())), "{e:?}");
    }

    #[test]
    fn checklist_entries_are_checked() {
        let p = format!(
            "{PROGRAM}
[[item]]
id = \"PRD-002\"
name = \"F\"
kind = \"feature\"
status = \"planned\"
priority = \"P0\"
parent = \"PRD-001\"
description = \"f\"
acceptance = [\"works\"]
due = \"2027-13-01\"
"
        );
        let e = rules(&check(&p, ""), Severity::Error);
        assert!(e.contains(&("R07", "PRD-002".into())));
        assert!(e.contains(&("R12", "PRD-002".into())));
    }
}
