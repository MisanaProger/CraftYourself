//! End-to-end test of the learning loop (architecture v0.2, §19):
//! project → task → workspace → mentor → own work in the IDE → Git → finish.

use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use est_api::{Config, Runtime, router};
use est_git::{Identity, Repo};
use est_workspace::EstHome;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

struct Harness {
    app: Router,
    rt: Runtime,
    _dir: tempfile::TempDir,
    origin: PathBuf,
    revision: String,
}

fn learner() -> Identity {
    Identity {
        name: "Learner".into(),
        email: "learner@example.invalid".into(),
    }
}

fn make_origin(dir: &Path) -> (PathBuf, String) {
    let repo = Repo::init(dir.join("ranges")).unwrap();
    let root = repo.root();
    std::fs::write(
        root.join("LICENSE"),
        "MIT License\n\nPermission is hereby granted, free of charge, to any person",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"ranges\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests/hidden")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn parse_range(s: &str) -> Vec<u32> {\n    let (a, b) = s.split_once(\"..\").unwrap();\n    (a.parse().unwrap()..b.parse().unwrap()).collect()\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tests/hidden/reference.rs"),
        "// fn parse_range( reference\n",
    )
    .unwrap();
    repo.add(&[".".into()]).unwrap();
    let rev = repo.commit("Initial import", &learner()).unwrap();
    (root.canonicalize().unwrap(), rev)
}

async fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let (origin, revision) = make_origin(dir.path());
    let rt = Runtime::build(EstHome::new(dir.path().join(".est")), Config::default())
        .await
        .unwrap();
    Harness {
        app: router(rt.clone(), None),
        rt,
        _dir: dir,
        origin,
        revision,
    }
}

impl Harness {
    async fn raw(&self, req: Request<Body>) -> (StatusCode, String) {
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn call(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "localhost:7878");
        let body = match body {
            Some(b) => {
                req = req.header("content-type", "application/json");
                Body::from(b.to_string())
            }
            None => Body::empty(),
        };
        let (s, text) = self.raw(req.body(body).unwrap()).await;
        (
            s,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }

    async fn get(&self, uri: &str) -> (StatusCode, Value) {
        self.call("GET", uri, None).await
    }

    async fn post(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.call("POST", uri, Some(body)).await
    }

    async fn new_project(&self) -> Value {
        let (s, p) = self
            .post("/api/v1/projects", json!({ "source": self.origin }))
            .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        p
    }
}

#[tokio::test]
async fn health() {
    let h = harness().await;
    let (s, v) = h.get("/api/v1/health").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["api"], "v1");
}

#[tokio::test]
async fn rejects_foreign_hosts_and_origins() {
    let h = harness().await;
    let req = Request::get("/api/v1/health")
        .header("host", "evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(h.raw(req).await.0, StatusCode::FORBIDDEN);
    let req = Request::post("/api/v1/projects")
        .header("host", "localhost:7878")
        .header("origin", "https://evil.example")
        .header("content-type", "application/json")
        .body(Body::from(json!({"source": "/tmp"}).to_string()))
        .unwrap();
    assert_eq!(h.raw(req).await.0, StatusCode::FORBIDDEN);
    let req = Request::get("/api/v1/health")
        .header("host", "localhost:7878")
        .header("origin", "http://localhost:7878")
        .body(Body::empty())
        .unwrap();
    assert_eq!(h.raw(req).await.0, StatusCode::OK);
}

#[tokio::test]
async fn connects_local_repository() {
    let h = harness().await;
    let mut events = h.rt.events.subscribe();
    let p = h.new_project().await;
    assert_eq!(p["name"], "ranges");
    assert_eq!(p["license"], "MIT");
    assert_eq!(p["primary_language"], "rust");
    assert_eq!(p["local_path"], h.origin.to_str().unwrap());
    assert_eq!(events.recv().await.unwrap().kind, "project.added");

    let (s, _) = h
        .post("/api/v1/projects", json!({ "source": h.origin }))
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = h
        .post(
            "/api/v1/projects",
            json!({ "source": "/definitely/missing" }),
        )
        .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);

    let id = p["id"].as_str().unwrap();
    let (_, cmds) = h.get(&format!("/api/v1/projects/{id}/commands")).await;
    let programs: Vec<_> = cmds
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["program"].as_str().unwrap())
        .collect();
    assert!(
        programs.iter().all(|p| *p == "cargo") && programs.len() == 4,
        "{cmds}"
    );
}

#[tokio::test]
async fn full_learning_loop() {
    let h = harness().await;
    let project = h.new_project().await;
    let pid = project["id"].as_str().unwrap();

    // Task.
    let (s, task) = h
        .post(
            "/api/v1/tasks",
            json!({
                "project_id": pid,
                "title": "Include the upper bound in parse_range",
                "description": "`parse_range(\"1..3\")` must return [1, 2, 3].",
                "task_type": "bug-fix",
                "difficulty": "beginner",
                "learning_goals": ["Reproduce a bug with a test"],
                "skills": ["debugging", "testing"],
                "base_revision": h.revision,
                "mentor": { "max_help_level": 4, "min_attempts_before_solution": 1, "denied_paths": ["tests/hidden/**"] }
            }),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{task}");
    let tid = task["id"].as_str().unwrap();

    // Start: a separate workspace on its own branch.
    let (s, started) = h
        .post(&format!("/api/v1/tasks/{tid}/start"), json!({}))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{started}");
    assert_eq!(started["resumed"], false);
    let sid = started["id"].as_str().unwrap().to_owned();
    let ws = PathBuf::from(started["workspace_path"].as_str().unwrap());
    assert!(ws.starts_with(h.rt.home.workspaces_dir()));
    assert!(
        started["branch"]
            .as_str()
            .unwrap()
            .starts_with("est/include-the-upper-bound")
    );
    let (s, again) = h
        .post(&format!("/api/v1/tasks/{tid}/start"), json!({}))
        .await;
    assert_eq!(
        (s, &again["id"], &again["resumed"]),
        (StatusCode::OK, &json!(sid), &json!(true))
    );
    let (_, t) = h.get(&format!("/api/v1/tasks/{tid}")).await;
    assert_eq!(t["status"], "in-progress");

    // Mentor before any own attempt.
    let base = format!("/api/v1/sessions/{sid}");
    let (_, m) = h
        .post(
            &format!("{base}/mentor/ask"),
            json!({ "question": "Покажи решение", "level": 4 }),
        )
        .await;
    assert_eq!(m["help"]["granted"], 2, "{m}");
    assert_eq!(m["constraints"]["code"], "illustrative");

    // API question: redirected to sources, hidden tests are not revealed.
    let (_, m) = h
        .post(
            &format!("{base}/mentor/ask"),
            json!({ "question": "Что возвращает функция `parse_range`?" }),
        )
        .await;
    assert_eq!(m["boundary"]["action"], "redirect");
    assert_eq!(m["forward_to_model"], false);
    let targets: Vec<_> = m["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["target"].as_str().unwrap())
        .collect();
    assert_eq!(targets, vec!["src/lib.rs:1"], "{m}");

    // The learner works in their own IDE and commits with their own git.
    let repo = Repo::open(&ws);
    let src = std::fs::read_to_string(ws.join("src/lib.rs")).unwrap();
    std::fs::write(
        ws.join("src/lib.rs"),
        src.replace("..b.parse", "..=b.parse"),
    )
    .unwrap();
    let (_, overview) = h.get(&base).await;
    assert_eq!(
        overview["changes"],
        json!([{ "code": " M", "path": "src/lib.rs" }])
    );
    let (_, diff) = h.get(&format!("{base}/diff")).await;
    assert!(diff.as_str().unwrap().contains("..=b.parse"));
    repo.add(&["src/lib.rs".into()]).unwrap();
    repo.commit("Include upper bound in parse_range", &learner())
        .unwrap();

    let (_, overview) = h.get(&base).await;
    assert_eq!(
        overview["commits"][0]["subject"],
        "Include upper bound in parse_range"
    );
    assert_eq!(overview["changes"], json!([]));
    assert_eq!(
        overview["task"]["title"],
        "Include the upper bound in parse_range"
    );
    let (_, diff) = h.get(&format!("{base}/diff?against=base")).await;
    assert!(diff.as_str().unwrap().contains("..=b.parse"));

    // One own commit unlocks solution-level help for this task.
    let (_, m) = h
        .post(
            &format!("{base}/mentor/ask"),
            json!({ "question": "Покажи решение", "level": 4 }),
        )
        .await;
    assert_eq!(m["help"]["granted"], 4, "{m}");
    let (_, history) = h.get(&format!("{base}/mentor/history")).await;
    assert_eq!(history.as_array().unwrap().len(), 3);
    assert_eq!(history[0]["payload"]["question"], "Покажи решение");

    // Finish.
    let (s, done) = h
        .post(&format!("{base}/finish"), json!({ "outcome": "completed" }))
        .await;
    assert_eq!(s, StatusCode::OK, "{done}");
    assert_eq!(done["status"], "completed");
    let (s, _) = h
        .post(&format!("{base}/finish"), json!({ "outcome": "completed" }))
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (_, t) = h.get(&format!("/api/v1/tasks/{tid}")).await;
    assert_eq!(t["status"], "done");

    let (_, events) = h.get(&format!("{base}/events")).await;
    let kinds: Vec<_> = events
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "session.started",
            "session.resumed",
            "mentor.request",
            "mentor.request",
            "mentor.request",
            "session.finished"
        ]
    );
    assert_eq!(events[5]["payload"]["commits"], 1);

    // Starting again continues in the same workspace.
    let (s, next) = h
        .post(&format!("/api/v1/tasks/{tid}/start"), json!({}))
        .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(next["workspace_path"], started["workspace_path"]);
}

#[tokio::test]
async fn imports_exercise_manifest() {
    let h = harness().await;
    let project = h.new_project().await;
    let manifest = format!(
        r#"manifest_version = 1
[exercise]
id = "inclusive-range"
version = "1.0.0"
title = "Inclusive ranges"
description = "Fix it."
task_type = "bug-fix"
difficulty = "beginner"
learning_goals = ["Read code"]
skills = ["code-reading"]
[source]
repository = "file://{}"
revision = "{}"
license = "MIT"
language = "rust"
[commands]
test = "cargo test"
[[criteria]]
id = "tests"
kind = "command"
description = "Tests pass"
command = "cargo test"
"#,
        h.origin.display(),
        h.revision
    );
    let (s, task) = h
        .post("/api/v1/exercises/import", json!({ "manifest": manifest }))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{task}");
    assert_eq!(
        task["project_id"], project["id"],
        "reuses the connected project"
    );
    assert_eq!(
        task["source"],
        json!({ "type": "exercise", "id": "inclusive-range", "version": "1.0.0" })
    );
    assert_eq!(task["test_command"], "cargo test");

    let (_, again) = h
        .post("/api/v1/exercises/import", json!({ "manifest": manifest }))
        .await;
    assert_eq!(again["id"], task["id"], "import is idempotent");

    let bad = manifest.replace(&h.revision, "main");
    let (s, err) = h
        .post("/api/v1/exercises/import", json!({ "manifest": bad }))
        .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(err["error"].as_str().unwrap().contains("source.revision"));
}
