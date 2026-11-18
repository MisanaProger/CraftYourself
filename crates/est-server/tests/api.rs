//! End-to-end test of the Local Edition API against a local git repository.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use est_git::{Identity, Repo};
use est_server::{router, AppState, Config};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

struct Harness {
    app: Router,
    _dir: tempfile::TempDir,
    origin: String,
    revision: String,
}

fn make_origin(dir: &Path) -> (String, String) {
    let repo = Repo::init(dir.join("origin")).unwrap();
    let root = repo.root();
    std::fs::write(
        root.join("LICENSE"),
        "MIT License\n\nPermission is hereby granted, free of charge, to any person...",
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
        "// fn parse_range( reference solution\n",
    )
    .unwrap();
    repo.add(&[".".into()]).unwrap();
    let rev = repo
        .commit(
            "Initial import",
            &Identity {
                name: "Upstream".into(),
                email: "up@example.invalid".into(),
            },
        )
        .unwrap();
    (format!("file://{}", root.display()), rev)
}

async fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let (origin, revision) = make_origin(dir.path());
    let config = Config {
        data_dir: dir.path().join("data"),
        bind: "127.0.0.1:0".parse().unwrap(),
        allow_local_repos: true,
    };
    let state = AppState::init(config).await.unwrap();
    Harness {
        app: router(state),
        _dir: dir,
        origin,
        revision,
    }
}

impl Harness {
    async fn call(
        &self,
        method: &str,
        uri: &str,
        body: Option<Body>,
        content_type: &str,
    ) -> (StatusCode, Vec<u8>) {
        let mut req = Request::builder().method(method).uri(uri);
        if body.is_some() {
            req = req.header("content-type", content_type);
        }
        let res = self
            .app
            .clone()
            .oneshot(req.body(body.unwrap_or_else(Body::empty)).unwrap())
            .await
            .unwrap();
        let status = res.status();
        (
            status,
            res.into_body().collect().await.unwrap().to_bytes().to_vec(),
        )
    }

    async fn json(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let (s, bytes) = self
            .call(
                method,
                uri,
                body.map(|b| Body::from(b.to_string())),
                "application/json",
            )
            .await;
        (s, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn text(&self, method: &str, uri: &str, body: Option<&str>) -> (StatusCode, String) {
        let (s, bytes) = self
            .call(
                method,
                uri,
                body.map(|b| Body::from(b.to_owned())),
                "text/plain",
            )
            .await;
        (s, String::from_utf8(bytes).unwrap())
    }

    fn manifest(&self) -> String {
        format!(
            r#"manifest_version = 1

[exercise]
id = "fix-inclusive-range"
version = "1.0.0"
title = "Fix inclusive range parsing"
description = "`parse_range(\"1..3\")` must include the upper bound."
category = "bug-fix"
difficulty = "beginner"
learning_goals = ["Reproduce a bug with a failing test"]
competencies = ["rust.iterators"]

[source]
repository = "{}"
revision = "{}"
license = "MIT"
language = "rust"

[commands]
test = "cargo test"

[[criteria]]
id = "tests-pass"
kind = "command"
description = "All tests pass"
command = "cargo test"

[mentor]
max_help_level = 4
min_attempts_before_demonstration = 1
denied_paths = ["tests/hidden/**"]

[[docs]]
title = "RangeInclusive"
url = "https://doc.rust-lang.org/std/ops/struct.RangeInclusive.html"
topics = ["RangeInclusive"]
"#,
            self.origin, self.revision
        )
    }
}

#[tokio::test]
async fn health_and_local_user() {
    let h = harness().await;
    let (s, v) = h.json("GET", "/api/health", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["status"], "ok");
    let (s, v) = h.json("GET", "/api/me", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["is_local"], true);
}

#[tokio::test]
async fn connect_project() {
    let h = harness().await;
    let (s, v) = h
        .json("POST", "/api/projects", Some(json!({ "url": h.origin })))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["license"], "MIT");
    assert_eq!(v["language"], "rust");
    assert_eq!(v["head_revision"], h.revision);
    assert!(v.get("license_warning").is_none());
    assert!(v.get("local_path").is_none(), "server paths must not leak");

    let id = v["id"].as_str().unwrap();
    let (_, files) = h
        .json("GET", &format!("/api/projects/{id}/files"), None)
        .await;
    assert!(files.as_array().unwrap().iter().any(|f| f == "src/lib.rs"));

    let (s, _) = h
        .json("POST", "/api/projects", Some(json!({ "url": h.origin })))
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
}

#[tokio::test]
async fn rejects_disallowed_urls() {
    let h = harness().await;
    let (s, _) = h
        .json(
            "POST",
            "/api/projects",
            Some(json!({ "url": "ext::sh -c id" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn invalid_manifest_is_rejected() {
    let h = harness().await;
    let bad = h.manifest().replace(&h.revision, "main");
    let (s, body) = h.text("POST", "/api/exercises", Some(&bad)).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("source.revision"), "{body}");
}

#[tokio::test]
async fn full_learning_session() {
    let h = harness().await;

    // Register the exercise.
    let (s, body) = h.text("POST", "/api/exercises", Some(&h.manifest())).await;
    assert_eq!(s, StatusCode::CREATED, "{body}");

    // Start a session: the project is cloned on demand.
    let (s, session) = h
        .json(
            "POST",
            "/api/sessions",
            Some(json!({ "exercise_id": "fix-inclusive-range" })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{session}");
    assert_eq!(session["branch"], "est/fix-inclusive-range");
    assert_eq!(session["base_revision"], h.revision);
    let sid = session["id"].as_str().unwrap().to_owned();
    let base = format!("/api/sessions/{sid}");

    let (_, branches) = h.json("GET", &format!("{base}/git/branches"), None).await;
    assert_eq!(branches["current"], "est/fix-inclusive-range");

    // Edit a file through the workspace API.
    let (s, src) = h
        .text("GET", &format!("{base}/files/src/lib.rs"), None)
        .await;
    assert_eq!(s, StatusCode::OK);
    let fixed = src.replace("..b.parse", "..=b.parse");
    let (s, _) = h
        .text("PUT", &format!("{base}/files/src/lib.rs"), Some(&fixed))
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = h
        .text("PUT", &format!("{base}/files/../escape.rs"), Some("x"))
        .await;
    assert!(s.is_client_error());
    let (s, _) = h
        .text("GET", &format!("{base}/files/.git/config"), None)
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let (_, status) = h.json("GET", &format!("{base}/git/status"), None).await;
    assert_eq!(status, json!([{ "code": " M", "path": "src/lib.rs" }]));
    let (_, diff) = h.text("GET", &format!("{base}/git/diff"), None).await;
    assert!(diff.contains("..=b.parse"));

    // Commit.
    let (s, _) = h
        .json(
            "POST",
            &format!("{base}/git/add"),
            Some(json!({ "paths": ["src/lib.rs"] })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let (s, commit) = h
        .json(
            "POST",
            &format!("{base}/git/commit"),
            Some(json!({ "message": "Include upper bound in parse_range\n\nUse an inclusive range." })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{commit}");
    let (_, log) = h
        .json("GET", &format!("{base}/git/log?limit=5"), None)
        .await;
    assert_eq!(log[0]["subject"], "Include upper bound in parse_range");
    assert_eq!(log[1]["hash"], h.revision);
    let (_, diff) = h
        .text("GET", &format!("{base}/git/diff?against=base"), None)
        .await;
    assert!(diff.contains("..=b.parse"));

    // Mentor: an API-reference question is redirected to sources and never
    // forwarded to a model; the hidden reference test is not revealed.
    let (s, m) = h
        .json(
            "POST",
            &format!("{base}/mentor/route"),
            Some(json!({ "question": "Что возвращает функция `parse_range`?" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{m}");
    assert_eq!(m["kind"]["type"], "api_reference");
    assert_eq!(m["boundary"]["action"], "redirect");
    assert_eq!(m["forward_to_model"], false);
    let targets: Vec<&str> = m["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["target"].as_str().unwrap())
        .collect();
    assert_eq!(targets, vec!["src/lib.rs:1"], "{m}");

    // A conceptual question may go to the model.
    let (_, m) = h
        .json(
            "POST",
            &format!("{base}/mentor/route"),
            Some(json!({ "question": "Что такое итератор?", "level": 3 })),
        )
        .await;
    assert_eq!(m["kind"]["type"], "conceptual");
    assert_eq!(m["forward_to_model"], true);
    assert_eq!(m["help"]["granted"], 3);

    // Demonstration is unlocked after one own attempt (the commit above).
    let (_, m) = h
        .json(
            "POST",
            &format!("{base}/mentor/route"),
            Some(json!({ "question": "Покажи, как бы ты подошёл к задаче", "level": 4 })),
        )
        .await;
    assert_eq!(m["help"]["granted"], 4, "{m}");

    // Everything is recorded for analytics.
    let (_, events) = h.json("GET", &format!("{base}/events"), None).await;
    let kinds: Vec<&str> = events
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "session.started",
            "git.commit",
            "mentor.request",
            "mentor.request",
            "mentor.request"
        ]
    );

    let (s, done) = h
        .json(
            "POST",
            &format!("{base}/status"),
            Some(json!({ "status": "completed" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(done["status"], "completed");
}

#[tokio::test]
async fn demonstration_is_locked_without_attempts() {
    let h = harness().await;
    h.text("POST", "/api/exercises", Some(&h.manifest())).await;
    let (_, session) = h
        .json(
            "POST",
            "/api/sessions",
            Some(json!({ "exercise_id": "fix-inclusive-range" })),
        )
        .await;
    let sid = session["id"].as_str().unwrap();
    let (_, m) = h
        .json(
            "POST",
            &format!("/api/sessions/{sid}/mentor/route"),
            Some(json!({ "question": "Покажи решение", "level": 4 })),
        )
        .await;
    assert_eq!(m["help"]["granted"], 3);
    assert!(m["help"]["limited_by"]
        .as_str()
        .unwrap()
        .contains("attempts"));
}
