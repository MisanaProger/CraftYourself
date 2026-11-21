//! Progress from the items' own data (§7).
//!
//! Progress is never a count of closed items. A leaf (an item without
//! children) contributes its weight — `estimate_days`, or 1 when it has no
//! estimate — multiplied by:
//!
//! - 1 when it is Done;
//! - the share of met acceptance criteria while it is In Progress, In
//!   Review or Blocked (no criteria → 0);
//! - 0 otherwise. Cancelled leaves are left out of the total.
//!
//! Unestimated leaves are counted, so a figure built from guesses says so.

use serde::Serialize;

use crate::{Kind, Roadmap, Status, criterion};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Progress {
    /// Leaves under the item (the item itself if it has no children).
    pub leaves: u32,
    pub done: u32,
    pub active: u32,
    pub blocked: u32,
    pub cancelled: u32,
    /// Leaves without `estimate_days` (weighted as 1 day).
    pub unestimated: u32,
    pub weight_total: f64,
    pub weight_done: f64,
    pub criteria_met: u32,
    pub criteria_total: u32,
}

impl Progress {
    /// Effort-weighted completion, 0–100; `None` when nothing is in scope.
    pub fn percent(&self) -> Option<u32> {
        (self.weight_total > 0.0)
            .then(|| (self.weight_done / self.weight_total * 100.0).round() as u32)
    }

    fn add(&mut self, o: &Progress) {
        self.leaves += o.leaves;
        self.done += o.done;
        self.active += o.active;
        self.blocked += o.blocked;
        self.cancelled += o.cancelled;
        self.unestimated += o.unestimated;
        self.weight_total += o.weight_total;
        self.weight_done += o.weight_done;
        self.criteria_met += o.criteria_met;
        self.criteria_total += o.criteria_total;
    }
}

fn criteria(item: &crate::Item) -> (u32, u32) {
    let all: Vec<bool> = item
        .acceptance
        .iter()
        .filter_map(|c| criterion(c).map(|(m, _)| m))
        .collect();
    (all.iter().filter(|m| **m).count() as u32, all.len() as u32)
}

/// Progress of an item and everything below it.
pub fn progress(rm: &Roadmap, id: &str) -> Progress {
    progress_inner(rm, id, 0)
}

fn progress_inner(rm: &Roadmap, id: &str, depth: usize) -> Progress {
    let Some(e) = rm.get(id) else {
        return Progress::default();
    };
    let it = &e.item;
    let mut p = Progress::default();
    let (met, total) = criteria(it);
    let children: Vec<&str> = rm.children(id).map(|c| c.item.id.as_str()).collect();
    // depth guard: parent loops are reported by the validator
    if !children.is_empty() && depth < 16 {
        p.criteria_met += met;
        p.criteria_total += total;
        for c in children {
            p.add(&progress_inner(rm, c, depth + 1));
        }
        return p;
    }
    p.leaves = 1;
    p.criteria_met = met;
    p.criteria_total = total;
    if it.status == Status::Cancelled {
        p.cancelled = 1;
        return p;
    }
    let w = match it.estimate_days {
        Some(d) => d,
        None => {
            p.unestimated = 1;
            1.0
        }
    };
    p.weight_total = w;
    match it.status {
        Status::Done => {
            p.done = 1;
            p.weight_done = w;
        }
        s if s.is_active() => {
            p.active = 1;
            if s == Status::Blocked {
                p.blocked = 1;
            }
            if total > 0 {
                p.weight_done = w * f64::from(met) / f64::from(total);
            }
        }
        _ => {}
    }
    p
}

/// Planned versus actual effort over finished leaves that have both.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Effort {
    pub items: u32,
    pub estimated_days: f64,
    pub actual_days: f64,
}

impl Effort {
    /// Actual / estimated; above 1 means estimates were too low.
    pub fn ratio(&self) -> Option<f64> {
        (self.estimated_days > 0.0).then(|| self.actual_days / self.estimated_days)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseProgress {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub status: Status,
    pub due: Option<String>,
    pub items: Vec<String>,
    pub progress: Progress,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub items: u32,
    pub by_status: Vec<(Status, u32)>,
    pub blocked: Vec<String>,
    pub debt_open: Vec<String>,
    pub debt_days: f64,
    pub effort: Effort,
    pub programs: Vec<(String, Progress)>,
    pub epics: Vec<(String, Progress)>,
    pub releases: Vec<ReleaseProgress>,
    /// Committed features/tasks: acceptance criteria met / total.
    pub criteria_met: u32,
    pub criteria_total: u32,
}

/// Items not closed whose `due` is before `today` (`YYYY-MM-DD`).
pub fn overdue<'a>(rm: &'a Roadmap, today: &str) -> Vec<&'a crate::Entry> {
    rm.items
        .iter()
        .filter(|e| {
            !e.item.status.is_closed()
                && e.item.status != Status::Deferred
                && e.item.due.as_deref().is_some_and(|d| d < today)
        })
        .collect()
}

pub fn metrics(rm: &Roadmap) -> Metrics {
    let by_status = Status::ALL
        .iter()
        .map(|s| {
            (
                *s,
                rm.items.iter().filter(|e| e.item.status == *s).count() as u32,
            )
        })
        .filter(|(_, n)| *n > 0)
        .collect();
    let blocked = rm
        .items
        .iter()
        .filter(|e| e.item.status == Status::Blocked)
        .map(|e| e.item.id.clone())
        .collect();
    let debt: Vec<&crate::Entry> = rm
        .items
        .iter()
        .filter(|e| e.item.is_debt() && !e.item.status.is_closed())
        .collect();
    let mut effort = Effort::default();
    for e in &rm.items {
        let leaf = rm.children(&e.item.id).next().is_none();
        if let (true, Status::Done, Some(est), Some(act)) = (
            leaf,
            e.item.status,
            e.item.estimate_days,
            e.item.actual_days,
        ) {
            effort.items += 1;
            effort.estimated_days += est;
            effort.actual_days += act;
        }
    }
    let of_kind = |k: Kind| -> Vec<(String, Progress)> {
        rm.items
            .iter()
            .filter(|e| e.item.kind == k)
            .map(|e| (e.item.id.clone(), progress(rm, &e.item.id)))
            .collect()
    };
    let releases = rm
        .releases()
        .map(|r| {
            let items: Vec<String> = rm
                .items
                .iter()
                .filter(|e| e.item.release.as_deref() == Some(r.item.id.as_str()))
                .map(|e| e.item.id.clone())
                .collect();
            let mut p = Progress::default();
            for i in &items {
                // an item whose ancestor also targets this release is
                // already counted through the ancestor
                let mut counted = false;
                let mut cur = rm.get(i).and_then(|e| e.item.parent.clone());
                let mut guard = 0;
                while let Some(pid) = cur {
                    if items.contains(&pid) {
                        counted = true;
                        break;
                    }
                    guard += 1;
                    if guard > 16 {
                        break;
                    }
                    cur = rm.get(&pid).and_then(|e| e.item.parent.clone());
                }
                if !counted {
                    p.add(&progress(rm, i));
                }
            }
            ReleaseProgress {
                id: r.item.id.clone(),
                name: r.item.name.clone(),
                version: r.item.version.clone(),
                status: r.item.status,
                due: r.item.due.clone(),
                items,
                progress: p,
            }
        })
        .collect();
    let (mut met, mut total) = (0, 0);
    for e in &rm.items {
        if e.item.kind.is_work() && e.item.status.is_committed() {
            let (m, t) = criteria(&e.item);
            met += m;
            total += t;
        }
    }
    Metrics {
        items: rm.items.len() as u32,
        by_status,
        blocked,
        debt_days: debt
            .iter()
            .filter_map(|e| e.item.estimate_days)
            .fold(0.0, |a, b| a + b),
        debt_open: debt.iter().map(|e| e.item.id.clone()).collect(),
        effort,
        programs: of_kind(Kind::Program),
        epics: of_kind(Kind::Epic),
        releases,
        criteria_met: met,
        criteria_total: total,
    }
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
"#;

    #[test]
    fn progress_is_weighted_by_effort_and_criteria() {
        let p = r#"
[[item]]
id = "PRD-001"
name = "E"
kind = "epic"
status = "in-progress"
priority = "P0"
description = "e"
[[item]]
id = "PRD-002"
name = "Big, half done"
kind = "feature"
status = "in-progress"
priority = "P0"
parent = "PRD-001"
estimate_days = 8.0
acceptance = ["[x] a", "[ ] b"]
[[item]]
id = "PRD-003"
name = "Small, done"
kind = "feature"
status = "done"
priority = "P0"
parent = "PRD-001"
estimate_days = 2.0
actual_days = 3.0
acceptance = ["[x] a"]
[[item]]
id = "PRD-004"
name = "Dropped"
kind = "feature"
status = "cancelled"
priority = "P0"
parent = "PRD-001"
estimate_days = 50.0
"#;
        let rm = from_sources(CONFIG, &[("product.toml", p)]).unwrap();
        let pr = progress(&rm, "PRD-001");
        // (8 × 1/2 + 2) / 10 = 60 %, although one of two open features is done
        assert_eq!(pr.percent(), Some(60));
        assert_eq!((pr.leaves, pr.done, pr.cancelled), (3, 1, 1));
        let m = metrics(&rm);
        assert_eq!(m.effort.ratio(), Some(1.5));
        assert_eq!((m.criteria_met, m.criteria_total), (2, 3));
    }

    #[test]
    fn overdue_ignores_closed_items() {
        let p = r#"
[[item]]
id = "PRD-001"
name = "Late"
kind = "program"
status = "planned"
priority = "P0"
description = "e"
due = "2026-01-01"
[[item]]
id = "PRD-002"
name = "Finished late"
kind = "program"
status = "done"
priority = "P0"
description = "e"
due = "2026-01-01"
"#;
        let rm = from_sources(CONFIG, &[("product.toml", p)]).unwrap();
        let o: Vec<&str> = overdue(&rm, "2026-06-01")
            .iter()
            .map(|e| e.item.id.as_str())
            .collect();
        assert_eq!(o, ["PRD-001"]);
    }
}
