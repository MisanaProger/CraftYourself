//! End-to-end test of the learning loop (architecture v0.2, §19):
//! project → task → workspace → mentor → own work in the IDE → Git → finish.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use est_api::{Config, Runtime, router};
use est_application::AppResult;
use est_application::ports::{LlmProvider, LlmRequest, LlmResponse, ProviderInfo};
use est_git::{Identity, Repo};
use est_workspace::EstHome;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Records requests; always answers with a question and a code block, so
/// the policy's code limits can be observed.
#[derive(Default)]
struct FakeLlm {
    requests: Mutex<Vec<LlmRequest>>,
}

#[async_trait::async_trait]
impl LlmProvider for FakeLlm {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "fake".into(),
            model: "fake-1".into(),
            endpoint: "memory".into(),
            remote: false,
        }
    }

    async fn generate(&self, request: LlmRequest) -> AppResult<LlmResponse> {
        self.requests.lock().unwrap().push(request);
        Ok(LlmResponse {
            content: "What happens with the last element?\n```rust\nlet a = 1;\nlet b = 2;\n```"
                .into(),
            model: "fake-1".into(),
        })
    }
}

struct Harness {
    app: Router,
    rt: Runtime,
    llm: Arc<FakeLlm>,
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
    let llm = Arc::new(FakeLlm::default());
    let rt = Runtime::build_with(
        EstHome::new(dir.path().join(".est")),
        Config::default(),
        Some(llm.clone()),
    )
    .await
    .unwrap();
    Harness {
        app: router(rt.clone(), None),
        rt,
        llm,
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

    fn llm_calls(&self) -> usize {
        self.llm.requests.lock().unwrap().len()
    }

    async fn task(&self, pid: &str, title: &str) -> String {
        let (s, t) = self
            .post(
                "/api/v1/tasks",
                json!({
                    "project_id": pid, "title": title, "description": "Fix it.",
                    "task_type": "bug-fix", "difficulty": "beginner", "base_revision": self.revision,
                    "disciplines": ["debugging-testing"],
                    "mentor": { "max_level": 4, "denied_paths": ["tests/hidden/**"] }
                }),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{t}");
        t["id"].as_str().unwrap().to_owned()
    }

    /// Posts a message and returns the replies (without the user's message).
    async fn say(&self, conv: &str, text: &str) -> Vec<Value> {
        let (s, msgs) = self
            .post(
                &format!("/api/v1/conversations/{conv}/messages"),
                json!({ "content": text }),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{msgs}");
        let msgs = msgs.as_array().unwrap().clone();
        assert_eq!(msgs[0]["role"], "user");
        msgs[1..].to_vec()
    }

    async fn level(&self, conv: &str) -> u64 {
        self.get(&format!("/api/v1/conversations/{conv}/engagement"))
            .await
            .1["level"]
            .as_u64()
            .unwrap()
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
    let pid = project["id"].as_str().unwrap().to_owned();
    let tid = h.task(&pid, "Include the upper bound in parse_range").await;

    // Start: separate workspace, own branch, and the Issue's conversation at L0.
    let (s, started) = h
        .post(&format!("/api/v1/tasks/{tid}/start"), json!({}))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{started}");
    let ws = PathBuf::from(started["workspace_path"].as_str().unwrap());
    assert!(ws.starts_with(h.rt.home.workspaces_dir()));
    let conv = started["conversation_id"].as_str().unwrap().to_owned();
    let (_, c) = h.get(&format!("/api/v1/tasks/{tid}/conversation")).await;
    assert_eq!(c["id"], conv.as_str());
    assert_eq!(h.level(&conv).await, 0, "a new Issue starts at L0");

    // Within L0: the model answers, but code is cut by the policy.
    let r = h
        .say(
            &conv,
            "Я проверил, что пустая строка приходит из конфига. Что дальше?",
        )
        .await;
    assert_eq!(r[0]["role"], "assistant");
    assert_eq!(r[0]["level"], 0);
    assert!(
        !r[0]["content"].as_str().unwrap().contains("let a"),
        "{r:?}"
    );
    assert_eq!(r[0]["cards"][0]["type"], "policy_notice");
    assert_eq!(h.llm_calls(), 1);
    assert!(h.llm.requests.lock().unwrap()[0].system.contains("L0"));

    // Above the level: blocked, one-step escalation proposed, no model call.
    let r = h.say(&conv, "Дай полное решение").await;
    assert_eq!(r[0]["role"], "system");
    let card = &r[0]["cards"][0];
    assert_eq!(
        (
            card["type"].as_str(),
            card["from"].as_u64(),
            card["to"].as_u64()
        ),
        (Some("escalation"), Some(0), Some(1))
    );
    assert_eq!(
        h.llm_calls(),
        1,
        "a blocked request never reaches the model"
    );
    let esc = card["escalation_id"].as_str().unwrap().to_owned();

    // Consent raises one step; the request is re-processed and needs more.
    let (s, r) = h
        .post(
            &format!("/api/v1/engagement/escalations/{esc}/accept"),
            json!({}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{r}");
    assert_eq!(h.level(&conv).await, 1);
    assert_eq!(r[0]["cards"][0]["to"], 2);
    let (s, _) = h
        .post(
            &format!("/api/v1/engagement/escalations/{esc}/accept"),
            json!({}),
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "consent cannot be reused");
    let esc2 = r[0]["cards"][0]["escalation_id"]
        .as_str()
        .unwrap()
        .to_owned();
    h.post(
        &format!("/api/v1/engagement/escalations/{esc2}/accept"),
        json!({}),
    )
    .await;
    assert_eq!(h.level(&conv).await, 2);

    // Declining answers within the current level (L2).
    let (_, view) = h.get(&format!("/api/v1/conversations/{conv}")).await;
    let esc3 = view["pending_escalation"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, r) = h
        .post(
            &format!("/api/v1/engagement/escalations/{esc3}/reject"),
            json!({}),
        )
        .await;
    assert_eq!(
        (r[0]["role"].as_str(), r[0]["level"].as_u64()),
        (Some("assistant"), Some(2))
    );
    assert_eq!(h.level(&conv).await, 2);
    assert_eq!(h.llm_calls(), 2);
    let (_, view) = h.get(&format!("/api/v1/conversations/{conv}")).await;
    assert!(view["pending_escalation"].is_null());
    let statuses: Vec<_> = view["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["cards"].as_array().unwrap().clone())
        .filter(|c| c["type"] == "escalation")
        .map(|c| c["status"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(statuses, ["accepted", "accepted", "rejected"]);

    // Documentation Boundary: sources, no model, hidden tests stay hidden.
    let r = h.say(&conv, "Что возвращает функция `parse_range`?").await;
    assert_eq!(r[0]["cards"][0]["type"], "sources");
    let targets: Vec<_> = r[0]["cards"][0]["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["target"].as_str().unwrap())
        .collect();
    assert_eq!(targets, ["src/lib.rs:1"]);
    assert_eq!(h.llm_calls(), 2);

    // A new Issue starts at L0 again; the previous level is not inherited.
    let other = h.task(&pid, "Second issue").await;
    let (_, c2) = h.get(&format!("/api/v1/tasks/{other}/conversation")).await;
    let c2 = c2["id"].as_str().unwrap();
    assert_eq!(h.level(c2).await, 0);
    let (s, r) = h
        .post(
            &format!("/api/v1/conversations/{c2}/engagement/raise"),
            json!({}),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{r}");
    assert_eq!(h.level(c2).await, 1, "learner-initiated raise is one step");
    assert_eq!(h.level(&conv).await, 2);

    // Own work in the IDE, visible through Git.
    let repo = Repo::open(&ws);
    let src = std::fs::read_to_string(ws.join("src/lib.rs")).unwrap();
    std::fs::write(
        ws.join("src/lib.rs"),
        src.replace("..b.parse", "..=b.parse"),
    )
    .unwrap();
    repo.add(&["src/lib.rs".into()]).unwrap();
    repo.commit("Include upper bound in parse_range", &learner())
        .unwrap();
    let sid = started["id"].as_str().unwrap();
    let (_, diff) = h
        .get(&format!("/api/v1/sessions/{sid}/diff?against=base"))
        .await;
    assert!(diff.as_str().unwrap().contains("..=b.parse"));

    // The conversation of an Issue in progress cannot be deleted.
    let (s, _) = h
        .call("DELETE", &format!("/api/v1/conversations/{conv}"), None)
        .await;
    assert_eq!(s, StatusCode::CONFLICT);

    // Completing the Issue sets the final ADI.
    let (s, done) = h
        .post(&format!("/api/v1/tasks/{tid}/complete"), json!({}))
        .await;
    assert_eq!(s, StatusCode::OK, "{done}");
    let (_, adi) = h.get("/api/v1/analytics/adi").await;
    assert_eq!(adi["aggregate"]["issues"], 1);
    assert_eq!(adi["aggregate"]["provisional"], true);
    let item = &adi["history"][0];
    assert_eq!(item["scoring_version"], "observed-0.1");
    assert_eq!(
        item["observed_features"]["answers_by_level"],
        json!([1, 0, 1, 0, 0])
    );
    assert_eq!(item["observed_features"]["own_commits"], 1);
    assert_eq!(item["observed_features"]["escalations_accepted"], 2);
    assert!(
        adi["disclaimer"]
            .as_str()
            .unwrap()
            .contains("не является оценкой")
    );
    let (s, _) = h
        .post(&format!("/api/v1/tasks/{other}/complete"), json!({}))
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "no active session");

    // The Skill Matrix is separate from ADI: untested skills are not zero.
    let (_, m) = h.get("/api/v1/analytics/disciplines").await;
    let dt = m["disciplines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "debugging-testing")
        .unwrap();
    assert_eq!(
        (
            dt["status"].as_str(),
            &dt["score"],
            dt["practice_tasks"].as_u64()
        ),
        (Some("not_assessed"), &Value::Null, Some(1))
    );

    // Privacy: provider shown, export contains the history, delete removes it.
    let (_, p) = h.get("/api/v1/settings/privacy").await;
    assert_eq!(p["llm"]["remote"], false);
    let (s, export) = h.post("/api/v1/data/export", json!({})).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(export["format"], "craft-yourself-export/1");
    assert!(export["conversations"].as_array().unwrap().len() >= 2);
    let (s, _) = h
        .call("DELETE", &format!("/api/v1/conversations/{conv}"), None)
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = h.get(&format!("/api/v1/conversations/{conv}")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, adi) = h.get("/api/v1/analytics/adi").await;
    assert_eq!(
        adi["aggregate"]["issues"], 0,
        "related aggregates are deleted too"
    );
}

#[tokio::test]
async fn without_a_model_the_policy_still_applies() {
    let dir = tempfile::tempdir().unwrap();
    let (origin, _) = make_origin(dir.path());
    let rt = Runtime::build(EstHome::new(dir.path().join(".est")), Config::default())
        .await
        .unwrap();
    let app = router(rt, None);
    let req = |method: &str, uri: &str, body: Value| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "localhost:7878")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let json_of = |res: axum::response::Response| async move {
        serde_json::from_slice::<Value>(&res.into_body().collect().await.unwrap().to_bytes())
            .unwrap()
    };
    let p = json_of(
        app.clone()
            .oneshot(req("POST", "/api/v1/projects", json!({ "source": origin })))
            .await
            .unwrap(),
    )
    .await;
    let c = json_of(
        app.clone()
            .oneshot(req(
                "POST",
                "/api/v1/conversations",
                json!({ "project_id": p["id"] }),
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = c["id"].as_str().unwrap();
    let r = json_of(
        app.clone()
            .oneshot(req(
                "POST",
                &format!("/api/v1/conversations/{id}/messages"),
                json!({ "content": "Что такое итератор?" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r[1]["role"], "system");
    assert!(
        r[1]["content"]
            .as_str()
            .unwrap()
            .contains("модель ИИ не подключена")
    );
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
disciplines = ["debugging-testing"]
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
    assert_eq!(task["mentor"]["max_level"], 4);

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
