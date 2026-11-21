# Roadmap Management

How Craft Yourself plans and tracks its own development. Decision:
[ADR-0001](decisions/ADR-0001-roadmap-as-data.md). Entry point:
[`ROADMAP.md`](../ROADMAP.md).

## Principles

1. **One source of truth.** Items live once, in `roadmap/*.toml`. Every
   Markdown view under `docs/roadmaps/` and the summary in `ROADMAP.md` is
   generated from that data. A status changes in one place and shows the same
   everywhere.
2. **Stable ids.** An id is never reused or renamed, even when the item moves
   to another roadmap file. The prefix tells where the item was created, not
   where it lives now.
3. **Git-native.** Plain text, one item per table, reviewed in pull requests;
   history is `git log -p roadmap/`.
4. **Checked, not trusted.** `est-roadmap check` enforces the rules below;
   `cargo test --workspace` runs them on the repository, so CI rejects a
   broken plan or a stale view.
5. **Honest progress.** Progress comes from estimates and met acceptance
   criteria, never from the number of closed items. Code or files existing for
   an item is never evidence that it is done.

## Layout

```
roadmap/
  roadmap.toml          configuration: roadmaps, prefixes, project Definition of Done
  product.toml          PRD — Product
  technical.toml        TEC — Technical (architecture, components, debt)
  learning.toml         LRN — Learning methodology
  research.toml         RND — Research (hypothesis-driven)
  infrastructure.toml   INF — Infrastructure & Security
  releases.toml         REL — Releases, in shipping order
docs/roadmaps/          generated views (never edit)
docs/schemas/roadmap-schema.json
docs/decisions/         ADRs
docs/specifications/    specifications referenced by items
docs/audit/             repository audits
crates/est-roadmap      model, validator, metrics, renderer, CLI
```

A new roadmap type is a new `[[roadmap]]` entry in `roadmap.toml` and a new
file; no code changes.

## Items

```toml
[[item]]
id = "TEC-014"                       # PREFIX-NNN, stable
name = "Persistent evidence store"
kind = "feature"                     # program | epic | feature | task | subtask | release
status = "planned"                   # see Statuses
priority = "P0"                      # P0–P3 (not for releases)
parent = "TEC-011"                   # required below program
description = "…"                    # required for programs, epics, features, releases
depends_on = ["TEC-012"]             # links, see Links
release = "REL-002"                  # target release
acceptance = ["[ ] A new migration adds the evidence table"]
done_when = ["[ ] …"]                # item-specific Definition of Done
estimate_days = 3.0                  # optional; actual_days when done
risk = "medium"                      # low | medium | high
due = "2027-03-01"                   # YYYY-MM-DD
components = ["crates/est-storage", "migrations"]
docs = ["docs/specifications/evidence-assessment.md#record"]
tags = ["debt"]                      # `debt` marks technical debt
source = "TZ 2.1"                    # where the requirement comes from
status_reason = "…"                  # why blocked / deferred / cancelled / lagging
owner = "…"                          # defaults to default_owner
```

Research items (roadmaps with `hypothesis_driven = true`) replace acceptance
criteria with `hypothesis`, `accept_if`, `reject_if`, and record `result`
when finished. Releases have `version` and their own `acceptance` (release
criteria); their scope is every item whose `release` names them.

The fields map to TZ 5.5: Type → `kind` and `tags`; Related Components →
`components`; Related Documentation → `docs`; Estimated/Actual Effort →
`estimate_days`/`actual_days`; Risk Level → `risk`; Target Release →
`release`.

### Hierarchy

`Program → Epic → Feature → Task → Subtask`. Every item except programs and
releases has a parent of a higher kind (levels may be skipped, e.g. a task
directly under an epic). A parent may sit in another roadmap file, but
prefer `implements` for cross-roadmap work.

### Statuses

| Status | Meaning |
|---|---|
| Proposed | Suggested, not yet accepted into the plan |
| Planned | Accepted, not yet specified enough to start |
| Ready | Specified: acceptance criteria exist, dependencies are closed or known |
| In Progress | Being worked on |
| In Review | Done by the author, waiting for review |
| Blocked | A concrete dependency or problem stops it (`status_reason` or an open dependency) |
| Done | Every acceptance criterion and the Definition of Done are met |
| Deferred | Consciously postponed (`status_reason` required) |
| Cancelled | Will not be done (`status_reason` required) |

Acceptance criteria are checklists: `[ ] text` or `[x] text`. An item is Done
only when all of its criteria and `done_when` entries are `[x]` and the
project-wide `definition_of_done` in `roadmap.toml` holds.

### Priorities

P0 critical · P1 high value, needed for a complete product · P2 extension and
quality · P3 long-term and experimental. Priority does not decide the order
of work by itself: dependencies, risk and capacity do as well — see the
[bottlenecks](roadmaps/dependencies.md#bottlenecks).

### Links

| Field | Meaning |
|---|---|
| `depends_on` | Needs the target closed first |
| `blocks` | The target needs this one (the reverse of `depends_on`) |
| `related_to` | Informational |
| `implements` | This delivers the target (e.g. a TEC feature implements a PRD feature or an LRN method) |
| `validates` | This checks the target (experiments, test suites) |
| `extends` | Builds on the target without replacing it |
| `replaces` | Supersedes the target |

`depends_on` and `blocks` together form the dependency graph used for cycles,
readiness and release planning.

## Integrity rules

`cargo run -p est-roadmap -- check` (exit code 1 on errors; `--strict` also
fails on warnings).

| Rule | Severity | Checks |
|---|---|---|
| R01 | error | id is `PREFIX-NNN` with a configured prefix; prefixes are unique |
| R02 | error | ids are unique |
| R03 | error | name, priority (not releases), description (programs, epics, features, releases), release version; `status_reason` for Deferred, Cancelled, and Blocked without an open dependency |
| R04 | error | parent exists and is of a higher kind; programs and releases have none; no parent loops |
| R05 | error | links point at existing non-release items, not at themselves; `release` points at a release |
| R06 | error | no dependency cycles |
| R07 | error | checklist format; features, tasks and subtasks from Ready on have acceptance criteria (research: hypothesis, accept_if, reject_if) |
| R08 | error | Done only with every criterion met (research: with a result) |
| R09 | error / warning | a Done parent has no open children; Done work has closed dependencies (error); Ready or active work with open dependencies, or active work under a Proposed/Planned parent (warning) |
| R10 | error / warning | work does not depend on work planned for a later release (error) or on cancelled work (warning) |
| R11 | error | paths in `components` and `docs` are relative; for Done items they exist (a missing path of open work is a plan, not an error) |
| R12 | error | dates are `YYYY-MM-DD`; efforts are non-negative |
| R13 | error | a release is Done only when its scope is closed; nothing open targets a Done release |
| R14 | error | an item is not left Proposed/Planned/Ready when a Done item implements it, unless `status_reason` explains why; an item is not Done while something implementing it is open |

R14 is the synchronisation rule of TZ 5.9: “done in the Technical roadmap,
still planned in the Product roadmap, without explanation” is an error.

## Metrics

`docs/roadmaps/metrics.md` (generated) and `cargo run -p est-roadmap -- metrics`
(adds overdue items for today; `--json` for automation).

- **Progress** of an item = Σ over its leaves of weight × completion / Σ
  weight. Weight is `estimate_days` (1 when missing — the view then says
  “unestimated”). Completion is 1 for Done, the share of met acceptance
  criteria for In Progress / In Review / Blocked, 0 otherwise. Cancelled leaves
  leave the total. Closing many small items does not move a large epic.
- **Criteria met** — over committed features, tasks and subtasks.
- **Blocked, overdue, technical debt** (open items tagged `debt` with their
  estimates), **release progress** (each release's scope, each subtree counted
  once), **planned versus actual effort** (finished leaves with both numbers).

## Change management

Small changes (wording, criteria ticked, a task added) need only a commit.
A change to a program or an epic — scope, priority, release, dependencies, or
cancelling it — is recorded on the item:

```toml
[[item.changes]]
date = "2026-12-01"
summary = "Moved the competency graph view to 0.3"
reason = "The evidence ledger took longer; the view needs TEC-024"
impact = "REL-002 loses TEC-025; PRD-024 waits one release"
decision = "docs/decisions/ADR-0003-….md"   # when an architectural decision is involved
```

and, in the same pull request, the dependencies, target releases and related
documents (`docs`, specifications, `docs/architecture.md`) are updated.
Significant architectural decisions get an ADR in `docs/decisions/`
([template](decisions/ADR-0000-template.md)).

## Linking roadmap and code

`components` names directories, crates, modules, migrations, routes, tests or
documents. They are checked to exist for Done items and shown in the views.
Commits that implement an item mention its id in the body (`Refs TEC-014`).
Automated matching of items with repository state is planned
([TEC-091](roadmaps/technical.md#tec-091)); it will report signals, never
completion.

## Commands

```sh
cargo run -p est-roadmap -- check [--strict]
cargo run -p est-roadmap -- render [--check]
cargo run -p est-roadmap -- metrics [--today YYYY-MM-DD] [--json]
cargo run -p est-roadmap -- graph [--format mermaid|dot] [--all]
cargo run -p est-roadmap -- export          # JSON: config, items, findings
```
