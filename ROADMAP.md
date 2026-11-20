# Roadmap

Craft Yourself follows the stages of spec v0.3 (`docs/architecture.md`).
✅ done · ◐ partly done · ◯ planned.

## Stage status

| Stage | Status | Notes |
|---|---|---|
| 0. Foundation | ✅ | Cargo workspace, React app, REST + WebSocket, SQLite with migrations, config, logging, CI |
| 1. Project & Workspace | ✅ | Local/remote repositories, languages, license, per-Issue workspaces, IDE launch, Git status/diff |
| 2. Language & Sandbox | ✅ | Adapters (Rust, C, C++, Python, JS/TS), Docker/Podman sandbox with limits and isolation, test summaries |
| 3. Task & Issue Engine | ◐ | Tasks, exercise import, statuses, sessions, acceptance criteria. ◯ importing GitHub/GitLab Issues |
| 4. Chat-first Conversation Engine | ◐ | Conversations, cards, history, WebSocket events. ◯ streaming answers |
| 5. AI Gateway & Engagement | ◐ | OpenAI-compatible provider, L0–L4, consent, documentation boundary, network permission gate. ◯ gates for publishing Issues/PRs (no such actions yet) |
| 6. Conversation Intelligence | ◯ | Session Summary, Knowledge Profile, recommendations (export/delete ✅) |
| 7. ADI & Skill Matrix | ◐ | ADI from observed events with double decay; ESM catalog. ◯ evidence-based skill assessment, model-assisted ADI |
| 8. Interactive Learning | ◐ | Programs, lessons, quizzes, sandboxed artifacts, practice projects. ◯ more programs, embedded components |
| 9. Git Workflow & Analytics | ◯ | Commit quality review, PR preparation, Issue review |
| Desktop app | ◐ | Tauri 2 window with the embedded daemon and interface. ◯ signed installers, auto-update, tray, testing on Windows/macOS |

## Next

### Learning programs (tiles marked “coming soon”)

- **Git: commits and pull requests** — atomic commits, messages following the
  project's conventions, conflicts, preparing a PR (with the commit review of stage 9).
- **Debugging: hypotheses and reproduction** — minimal reproducible example,
  hypothesis log, debugger walkthroughs.
- **Reading unfamiliar code** — entry points, code search, module maps on a real project.
- **Working with documentation** — API references, versions, source as documentation.
- **Data structures, visualised** — hash tables, segment and Fenwick trees as
  interactive artifacts with exercises.
- **Issue engineering** — bug reports and feature requests with a quality checklist.
- Testing program, part 2: property-based testing (the `globset` exercise),
  test doubles, flaky tests.

### Product

- Session Summary after each Issue; Knowledge Profile and recommendations based on it.
- Evidence for the Skill Matrix: quiz and artifact results, sandbox runs,
  completed practice — with confidence and history.
- Streaming mentor answers over WebSocket.
- Commit review that learns the repository's own conventions; PR preparation.
- Import of open-source Issues with a suitability check.
- Localized exercise manifests (title, description, criteria in ru/en/uz).
- VS Code / VSCodium extension (optional).

### Platform

- Sandbox: gVisor/Kata option, per-language image presets, a dependency-fetch
  step declared in the manifest, Windows/macOS notes.
- PostgreSQL storage behind the same ports.
- Cloud Edition: accounts, remote sandboxes, groups (out of MVP scope).

## Open decisions (spec v0.3 §29)

Project license (the repository currently contains GPL-3.0), first LLM
providers to ship, sandbox system requirements, ADI coefficients and the
`T_t`/`T_n` calibration, the threshold for a stable ADI status, ESM weights,
disciplines in the first release.
