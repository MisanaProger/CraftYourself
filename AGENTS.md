# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository.
Product: **Craft Yourself** — a local-first trainer for engineering skills.
Internal names keep the `est` prefix (crates `est-*`, binary `est`, `~/.est`).
The specification is `docs/architecture.md` (spec v0.3); the plan is `ROADMAP.md`.

## Layout

```
crates/
  est-core           ids, DomainEvent, event names
  est-domain         models and invariants — no SQLx/Axum/Git/LLM/Docker here
  est-application    use cases + ports (traits); the only place that orchestrates
  est-ai-policy      what the mentor may do: classification, engagement, limits
  est-documentation  links to primary sources
  est-dependency     ADI scoring and double decay (pure)
  est-discipline     Skill Matrix catalog (pure, localized)
  est-exercises      Exercise Manifest (TOML)
  est-learning       learning content loader (embeds learning-artifacts/)
  est-problems       online-judge problems: links, statements, samples, workspace (pure)
  est-issues         GitHub/GitLab Issues: links, suitability check, task draft (pure)
  est-remote         forges and judges over HTTPS behind the RemoteSources port
  est-storage        SQLite repositories; migrations live in /migrations
  est-git            git CLI wrapper + GitPort
  est-sandbox        Docker/Podman runner + Sandbox port
  est-languages      language adapters (describe commands, never run them)
  est-projects       project inspection (languages, license)
  est-workspace      ~/.est layout, editor launcher
  est-ai             LLM providers (OpenAI-compatible, OpenAI, Claude) + presets
  est-api            Axum REST /api/v1, WebSocket, composition root (runtime.rs)
  est-cli            `est` binary
apps/desktop/        Tauri 2 desktop app (separate workspace): runs the daemon
                     in-process on a loopback port and shows it in a window;
                     the frontend is embedded; no Tauri IPC is exposed
frontend/            React + TypeScript + Vite (src/i18n, chat, learning, …)
learning-artifacts/  programs, lessons, quizzes, artifacts, practice templates
exercises/examples/  Exercise Manifests
migrations/          SQLx migrations (append-only)
```

Dependencies point inwards: `est-domain` ← `est-application` ← adapters ←
`est-api`/`est-cli`. A new infrastructure capability is a **port** (trait) in
`est-application/src/ports.rs` plus an implementation in an adapter crate,
wired in `est-api/src/runtime.rs`.

## Commands

```sh
cargo build
cargo test --workspace                                  # must pass
cargo clippy --workspace --all-targets -- -D warnings   # must be clean
cargo fmt --all
cd frontend && npm ci && npm run build                  # tsc + vite; must pass
EST_SANDBOX_IT=1 cargo test -p est-sandbox              # needs Docker/Podman
cd apps/desktop && cargo clippy -- -D warnings          # needs WebKitGTK + built frontend
```

Run the app: `cargo run --manifest-path apps/desktop/Cargo.toml` (desktop), or
`cargo run -p est-cli -- serve` with `EST_HOME=/tmp/est-dev` and
`EST_FRONTEND_DIR=frontend/dist` (browser). Both use the same daemon; keep the
desktop window free of Tauri IPC — features go through the HTTP API so the
browser build keeps working.

## Rules that must not be broken

1. **Policy lives in code, not in prompts.** Anything the mentor may or may
   not do (levels, escalation, documentation boundary, code limits, tool
   access) is decided in `est-ai-policy` / `est-application` before a model is
   called. The system prompt only restates it. A model never raises its own
   level or assumes consent.
2. **Every new Issue starts at L0**; escalation is one step and needs explicit
   consent (`accept`, or the learner's own `raise`). Acceptance criteria and
   translations are allowed at every level and carry no ADI weight.
3. **Untrusted code runs only in the sandbox.** Never execute project
   commands on the host. Keep the container flags (`--network none` by
   default, `--cap-drop ALL`, `--read-only`, `no-new-privileges`, limits,
   read-only workspace mount). Network for a run requires the learner's
   explicit permission.
4. **Learning artifacts are isolated.** They — and HTML pages written by a
   model (generated `page` lessons, ```html blocks in mentor answers) — are
   served with the CSP `sandbox allow-scripts` header (`routes/isolated.rs`)
   and shown in `<iframe sandbox="allow-scripts">` (`ui/IsolatedFrame.tsx`)
   — never add `allow-same-origin`, never render model HTML in the app's DOM. They talk to the platform only through
   the validated `postMessage` protocol (`est:progress`, `est:resize`, `version: 1`).
5. **The local daemon trusts only its own Host/Origin** (`est-api/src/guard.rs`).
   Mutating endpoints take JSON bodies.
6. **No secrets** in config files, events, messages, logs or exports. API keys
   come from environment variables named in config.
7. **Assessment uses observed events only.** ADI and the Skill Matrix stay
   independent; missing data is `not_assessed`, never zero.
8. **Migrations are append-only.** Never edit an applied migration; add a new file.
9. **Quiz answers never leave the server** before an attempt.

## Conventions

- Rust edition 2024; errors via `AppError`; blocking work (git, fs) inside
  `blocking(...)` in the application layer.
- Events: add names to `est_core::events` (or the module's `events`), attach
  `session/task/conversation` ids.
- Tests: domain rules get unit tests; user-visible flows get a test in
  `crates/est-api/tests/learning_loop.rs` (fake LLM and fake sandbox).
- Frontend: no business rules on the client; every UI string goes through
  `useT()`; keys are defined in `src/i18n/en.ts` and **must** exist in
  `ru.ts` and `uz.ts` (the compiler enforces it). Render untrusted text as
  React text (see `chat/Markdown.tsx`), never via `innerHTML`.
- Commits: imperative subject, body explains why; one logical change each.

## Common tasks

- **Add a lesson / program:** edit `learning-artifacts/catalog.toml` and
  `programs/<id>/program.toml`; texts as `lessons/<name>.{ru,en,uz}.md`,
  quizzes in `quizzes/*.toml`, practice templates under `practice/<name>/`
  (`Cargo.toml` is stored as `Cargo.toml.tmpl`; `exercise.toml` uses
  `{{repository}}` / `{{revision}}`). `cargo test -p est-learning` checks the
  content's consistency.
- **Add a language adapter:** implement `LanguageAdapter` in
  `crates/est-languages/src/adapters.rs`, register it in `LanguageRegistry`,
  add a sandbox image in `SandboxConfig::default()` and, if needed, a summary
  parser in `summary.rs`.
- **Add an API endpoint:** use case in `est-application`, route in
  `est-api/src/routes/`, test in `learning_loop.rs`, client in
  `frontend/src/services/api.ts`.

## Documentation

`README.md` (English) is the main README; `README.ru.md` and `README.uz.md`
are translations — update all three together. Architecture decisions go to
`docs/architecture.md`; planned work to `ROADMAP.md`.
