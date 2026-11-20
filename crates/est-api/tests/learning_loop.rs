//! End-to-end test of the learning loop (architecture v0.2, §19):
//! project → task → workspace → mentor → own work in the IDE → Git → finish.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use est_api::{Config, Runtime, router};
use est_application::AppResult;
use est_application::ports::{
    LlmProvider, LlmRequest, LlmResponse, ProviderInfo, Sandbox, SandboxInfo, SandboxOutcome,
    SandboxRequest,
};
use est_git::{Identity, Repo};
use est_workspace::EstHome;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Records requests; answers with a question and a code block (or `reply`,
/// when set), so the policy's code limits can be observed.
#[derive(Default)]
struct FakeLlm {
    requests: Mutex<Vec<LlmRequest>>,
    reply: Mutex<Option<String>>,
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
        let content = self.reply.lock().unwrap().clone().unwrap_or_else(|| {
            "What happens with the last element?\n```rust\nlet a = 1;\nlet b = 2;\n```".into()
        });
        Ok(LlmResponse {
            content,
            model: "fake-1".into(),
        })
    }
}

/// Records sandbox requests and pretends `cargo test` ran.
#[derive(Default)]
struct FakeSandbox {
    requests: Mutex<Vec<SandboxRequest>>,
    discarded: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl Sandbox for FakeSandbox {
    fn info(&self) -> SandboxInfo {
        SandboxInfo {
            runtime: Some("fake".into()),
            available: true,
            reason: None,
            limits: json!({}),
        }
    }

    async fn run(&self, request: SandboxRequest) -> AppResult<SandboxOutcome> {
        let failing = request.command.contains("fail");
        self.requests.lock().unwrap().push(request);
        Ok(SandboxOutcome {
            exit_code: Some(if failing { 101 } else { 0 }),
            stdout: if failing {
                "test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out"
                    .into()
            } else {
                "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out".into()
            },
            stderr: String::new(),
            duration_ms: 1200,
            timed_out: false,
            image: "fake-image".into(),
        })
    }

    async fn discard(&self, cache_key: &str) -> AppResult<()> {
        self.discarded.lock().unwrap().push(cache_key.into());
        Ok(())
    }
}

struct Harness {
    app: Router,
    rt: Runtime,
    llm: Arc<FakeLlm>,
    sandbox: Arc<FakeSandbox>,
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
    let sandbox = Arc::new(FakeSandbox::default());
    let rt = Runtime::build_with(
        EstHome::new(dir.path().join(".est")),
        Config::default(),
        Some(llm.clone()),
        sandbox.clone(),
    )
    .await
    .unwrap();
    Harness {
        app: router(rt.clone(), None),
        rt,
        llm,
        sandbox,
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

    async fn wait_run(&self, id: &str) -> Value {
        for _ in 0..100 {
            let (_, run) = self.get(&format!("/api/v1/sandbox/runs/{id}")).await;
            if run["status"] != "running" {
                return run;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("run {id} did not finish");
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

    // At L0 the mentor still formulates acceptance criteria and translates.
    let r = h
        .say(&conv, "Сформулируй критерии приёмки для этой задачи")
        .await;
    assert_eq!(
        (r[0]["role"].as_str(), r[0]["level"].as_u64()),
        (Some("assistant"), Some(0))
    );
    assert_eq!(
        r[0]["meta"]["decision"]["kind"]["type"],
        "acceptance_criteria"
    );
    let prompt = h
        .llm
        .requests
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .system
        .clone();
    assert!(
        prompt.contains("критерии приёмки") && !prompt.contains("отвечай только вопросами"),
        "{prompt}"
    );
    let r = h
        .say(
            &conv,
            "Переведи на русский: Returns the number of elements.",
        )
        .await;
    assert_eq!(r[0]["meta"]["decision"]["kind"]["type"], "translation");
    assert_eq!(h.llm_calls(), 3);
    assert_eq!(h.level(&conv).await, 0, "service requests never escalate");
    let (s, t) = h
        .call(
            "PUT",
            &format!("/api/v1/tasks/{tid}/acceptance-criteria"),
            Some(json!({ "criteria": [{ "text": "parse_range(\"1..3\") returns [1, 2, 3]" }, { "text": "  " }, { "text": "Regression test added", "done": true }] })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{t}");
    assert_eq!(t["acceptance_criteria"].as_array().unwrap().len(), 2);
    assert_eq!(t["acceptance_criteria"][1]["done"], true);

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
        3,
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
    assert_eq!(h.llm_calls(), 4);
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
    assert_eq!(h.llm_calls(), 4);

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

    // Tests run in the sandbox: asynchronously, offline, result in the chat.
    let (s, run) = h
        .post(
            &format!("/api/v1/sessions/{sid}/sandbox/runs"),
            json!({ "kind": "test" }),
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{run}");
    assert_eq!(run["status"], "running");
    let run = h.wait_run(run["id"].as_str().unwrap()).await;
    assert_eq!(run["status"], "passed", "{run}");
    assert_eq!(
        run["summary"],
        json!({ "passed": 3, "failed": 0, "ignored": 0 })
    );
    {
        let reqs = h.sandbox.requests.lock().unwrap();
        assert_eq!(
            reqs[0].command, "cargo test",
            "detected by the Rust adapter"
        );
        assert!(!reqs[0].network, "no network by default");
        assert_eq!(reqs[0].workspace, ws);
    }
    let (_, view) = h.get(&format!("/api/v1/conversations/{conv}")).await;
    let last = view["messages"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["cards"][0]["type"], "sandbox_run");
    assert_eq!(last["cards"][0]["summary"]["passed"], 3);
    let (s, _) = h
        .post(
            &format!("/api/v1/sessions/{sid}/sandbox/runs"),
            json!({ "kind": "setup" }),
        )
        .await;
    assert_eq!(
        s,
        StatusCode::UNPROCESSABLE_ENTITY,
        "network needs explicit permission"
    );
    let (_, run) = h
        .post(
            &format!("/api/v1/sessions/{sid}/sandbox/runs"),
            json!({ "kind": "custom", "command": "cargo test fail_case" }),
        )
        .await;
    let run = h.wait_run(run["id"].as_str().unwrap()).await;
    assert_eq!(
        (run["status"].as_str(), run["summary"]["failed"].as_u64()),
        (Some("failed"), Some(1))
    );
    let (_, runs) = h.get(&format!("/api/v1/sessions/{sid}/sandbox/runs")).await;
    assert_eq!(runs.as_array().unwrap().len(), 2);

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
    assert_eq!(item["observed_features"]["test_runs"], 1);
    assert_eq!(
        *h.sandbox.discarded.lock().unwrap(),
        vec![sid.to_owned()],
        "build cache removed with the session"
    );
    assert_eq!(
        item["observed_features"]["service_answers"], 2,
        "criteria and translation do not count as dependence"
    );
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
async fn connects_cloud_providers_without_leaking_keys() {
    let h = harness().await;
    let (s, presets) = h.get("/api/v1/llm/presets").await;
    assert_eq!(s, StatusCode::OK);
    let ids: Vec<&str> = presets
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    for must in ["claude", "chatgpt", "gemini", "deepseek", "kimi", "qwen"] {
        assert!(ids.contains(&must), "{must}");
    }

    // A remote provider must use https.
    let (s, _) = h
        .post(
            "/api/v1/llm/providers",
            json!({ "preset": "custom", "base_url": "http://api.example.com/v1", "model": "m" }),
        )
        .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);

    // Add Claude with a key: the key is stored, never returned.
    let (s, claude) = h
        .post(
            "/api/v1/llm/providers",
            json!({ "preset": "claude", "model": "claude-opus-5-5", "api_key": "sk-ant-secret" }),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{claude}");
    assert_eq!(claude["key"], "stored");
    assert_eq!(claude["remote"], true);
    assert_eq!(claude["active"], false);
    assert!(!claude.to_string().contains("sk-ant-secret"));
    let id = claude["id"].as_str().unwrap().to_owned();
    let config = std::fs::read_to_string(h.rt.home.config_dir().join("providers.toml")).unwrap();
    assert!(config.contains("claude-opus-5-5") && !config.contains("sk-ant-secret"));

    // A subscription plan without a key cannot be activated.
    let (_, kimi) = h
        .post(
            "/api/v1/llm/providers",
            json!({ "preset": "kimi-code", "model": "kimi-for-coding", "api_key_env": "EST_TEST_UNSET_KEY" }),
        )
        .await;
    assert_eq!(kimi["auth"], "subscription");
    assert_eq!(kimi["key"], "missing");
    let (s, _) = h
        .post("/api/v1/llm/active", json!({ "id": kimi["id"] }))
        .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, p) = h.get("/api/v1/settings/privacy").await;
    assert_eq!(p["llm"]["remote"], false, "the working provider is kept");

    // Activate Claude: the mentor now sends context to the cloud.
    let (s, list) = h.post("/api/v1/llm/active", json!({ "id": id })).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list["active"], id.as_str());
    let (_, p) = h.get("/api/v1/settings/privacy").await;
    assert_eq!(p["llm"]["remote"], true);
    assert_eq!(p["llm"]["name"], "Claude");
    assert_eq!(
        p["llm"]["endpoint"],
        "https://api.anthropic.com/v1/messages"
    );
    let (_, events) = h.get("/api/v1/sessions").await;
    assert!(!events.to_string().contains("sk-ant-secret"));

    // Changing the model applies at once; deleting removes key and model.
    let (s, _) = h
        .call(
            "PATCH",
            &format!("/api/v1/llm/providers/{id}"),
            Some(json!({ "model": "claude-sonnet-5-5" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let (_, p) = h.get("/api/v1/settings/privacy").await;
    assert_eq!(p["llm"]["model"], "claude-sonnet-5-5");
    let (s, _) = h
        .call("DELETE", &format!("/api/v1/llm/providers/{id}"), None)
        .await;
    assert_eq!(s, StatusCode::OK);
    assert!(!h.rt.home.key_store().has(&id));
    let (_, p) = h.get("/api/v1/settings/privacy").await;
    assert!(p["llm"].is_null());
    let (_, export) = h.post("/api/v1/data/export", json!({})).await;
    assert!(!export.to_string().contains("sk-ant-secret"));
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

#[tokio::test]
async fn testing_program() {
    let h = harness().await;
    let (_, tiles) = h.get("/api/v1/learning/programs?lang=en").await;
    let tiles = tiles.as_array().unwrap();
    let testing = tiles.iter().find(|t| t["id"] == "testing").unwrap();
    assert_eq!(
        (testing["status"].as_str(), testing["lessons"].as_u64()),
        (Some("available"), Some(6))
    );
    assert!(
        tiles
            .iter()
            .filter(|t| t["status"] == "coming_soon")
            .count()
            >= 5
    );
    let (_, uz) = h.get("/api/v1/learning/programs?lang=uz").await;
    assert_eq!(uz[0]["title"], "Testlar: ishga tushirish va sifat");
    let (s, _) = h.get("/api/v1/learning/programs/git-workflow").await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "coming soon");

    // Reading.
    let base = "/api/v1/learning/programs/testing/lessons";
    let (_, l) = h.get(&format!("{base}/run-tests?lang=en")).await;
    assert!(
        l["body"]["markdown"]
            .as_str()
            .unwrap()
            .starts_with("# Running tests")
    );
    assert_eq!(
        (l["next"].as_str(), &l["previous"]),
        (Some("read-output"), &Value::Null)
    );
    h.post(&format!("{base}/run-tests/complete"), json!({}))
        .await;

    // Quiz: no answers are sent before an attempt; graded on the server.
    let (_, l) = h.get(&format!("{base}/read-output?lang=ru")).await;
    let questions = l["body"]["questions"].as_array().unwrap();
    assert!(!l["body"].to_string().contains("\"answer\""));
    let (_, r) = h
        .post(
            &format!("{base}/read-output/quiz"),
            json!({ "answers": {} }),
        )
        .await;
    assert_eq!(
        (r["score"].as_u64(), r["passed"].as_bool()),
        (Some(0), Some(false))
    );
    let answers: serde_json::Map<String, Value> = r["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| (x["id"].as_str().unwrap().to_owned(), x["answer"].clone()))
        .collect();
    assert_eq!(answers.len(), questions.len());
    let (_, r) = h
        .post(
            &format!("{base}/read-output/quiz?lang=en"),
            json!({ "answers": answers }),
        )
        .await;
    assert_eq!(
        (r["score"].as_u64(), r["passed"].as_bool()),
        (Some(100), Some(true))
    );
    let (s, _) = h
        .post(&format!("{base}/run-tests/quiz"), json!({ "answers": {} }))
        .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "not a quiz lesson");

    // Artifact: an isolated document; progress is relayed by the platform.
    let req = Request::get("/api/v1/learning/artifacts/boundary-values/index.html")
        .header("host", "localhost:7878")
        .body(Body::empty())
        .unwrap();
    let res = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let csp = res.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        csp.starts_with("sandbox allow-scripts;") && csp.contains("connect-src 'none'"),
        "{csp}"
    );
    assert!(!csp.contains("allow-same-origin"));
    let (s, _) = h
        .get("/api/v1/learning/artifacts/boundary-values/..%2F..%2Fcatalog.toml")
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, p) = h
        .post(
            &format!("{base}/boundaries/progress"),
            json!({ "score": 140, "completed": true }),
        )
        .await;
    assert_eq!(
        (p["status"].as_str(), p["score"].as_u64()),
        (Some("completed"), Some(100))
    );

    // Practice: a real project with a bug, its own workspace, tests in the sandbox.
    let (s, started) = h
        .post(&format!("{base}/regression/practice"), json!({}))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{started}");
    let ws = PathBuf::from(started["workspace_path"].as_str().unwrap());
    assert!(
        std::fs::read_to_string(ws.join("src/lib.rs"))
            .unwrap()
            .contains("fn is_leap_year")
    );
    assert!(ws.join("Cargo.toml").is_file());
    let tid = started["task_id"].as_str().unwrap().to_owned();
    let (_, task) = h.get(&format!("/api/v1/tasks/{tid}")).await;
    assert_eq!(task["acceptance_criteria"].as_array().unwrap().len(), 4);
    assert_eq!(task["test_command"], "cargo test");
    let (_, again) = h
        .post(&format!("{base}/regression/practice"), json!({}))
        .await;
    assert_eq!(
        (again["id"].as_str(), again["resumed"].as_bool()),
        (started["id"].as_str(), Some(true))
    );
    let sid = started["id"].as_str().unwrap();
    let (_, run) = h
        .post(&format!("/api/v1/sessions/{sid}/sandbox/runs"), json!({}))
        .await;
    h.wait_run(run["id"].as_str().unwrap()).await;
    assert_eq!(
        h.sandbox.requests.lock().unwrap()[0].language.as_deref(),
        Some("rust")
    );
    let (_, l) = h.get(&format!("{base}/regression")).await;
    assert_eq!(
        (l["status"].as_str(), l["body"]["session_id"].as_str()),
        (Some("started"), Some(sid))
    );
    h.post(&format!("/api/v1/tasks/{tid}/complete"), json!({}))
        .await;

    let (_, view) = h.get("/api/v1/learning/programs/testing").await;
    let statuses: Vec<_> = view["lessons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        statuses,
        [
            "completed",
            "completed",
            "not_started",
            "not_started",
            "completed",
            "completed"
        ]
    );
    let (_, tiles) = h.get("/api/v1/learning/programs").await;
    assert_eq!(tiles[0]["completed"], 4);
    let (_, export) = h.post("/api/v1/data/export", json!({})).await;
    assert_eq!(export["learning_progress"].as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn mentor_html_pages_from_l2() {
    let h = harness().await;
    let (_, c) = h.post("/api/v1/conversations", json!({})).await;
    let conv = c["id"].as_str().unwrap();
    *h.llm.reply.lock().unwrap() = Some(
        "A heap keeps the minimum on top: $a_i \\le a_{2i+1}$.\n```html\n<!DOCTYPE html><html><body>heap</body></html>\n```\nWhy?"
            .into(),
    );
    let page_status = |mid: String, n: usize| {
        let h = &h;
        async move {
            h.get(&format!(
                "/api/v1/conversations/{conv}/messages/{mid}/pages/{n}"
            ))
            .await
            .0
        }
    };

    // Below L2 a page is removed even for a concept; the rest stays.
    let replies = h.say(conv, "What is a binary heap?").await;
    let m = &replies[0];
    assert_eq!(m["role"], "assistant", "{m}");
    let text = m["content"].as_str().unwrap();
    assert!(
        !text.contains("```html") && text.contains("$a_i") && text.ends_with("Why?"),
        "{text}"
    );
    assert_eq!(m["cards"][0]["code"], "page_removed");
    assert_eq!(m["meta"]["pages"], 0);
    assert_eq!(
        page_status(m["id"].as_str().unwrap().into(), 0).await,
        StatusCode::NOT_FOUND
    );
    let system = h.llm.requests.lock().unwrap()[0].system.clone();
    assert!(
        system.contains("LaTeX") && system.contains("недоступны"),
        "{system}"
    );

    // The learner raises the level to L2 step by step.
    for _ in 0..2 {
        h.post(
            &format!("/api/v1/conversations/{conv}/engagement/raise"),
            json!({}),
        )
        .await;
    }
    assert_eq!(h.level(conv).await, 2);
    let replies = h.say(conv, "What is a binary heap?").await;
    let m = &replies[0];
    assert_eq!(m["meta"]["pages"], 1, "{m}");
    assert!(m["content"].as_str().unwrap().contains("```html"));
    let mid = m["id"].as_str().unwrap().to_owned();
    let req = Request::get(format!(
        "/api/v1/conversations/{conv}/messages/{mid}/pages/0"
    ))
    .header("host", "localhost:7878")
    .body(Body::empty())
    .unwrap();
    let res = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let csp = res.headers()["content-security-policy"].to_str().unwrap();
    assert!(csp.starts_with("sandbox allow-scripts;") && !csp.contains("allow-same-origin"));
    assert!(csp.contains("connect-src 'none'"));
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body.to_vec()).unwrap();
    // The compatibility layer runs before the page's own content.
    assert!(body.starts_with("<!DOCTYPE html><html><script>"), "{body}");
    assert_eq!(page_status(mid, 1).await, StatusCode::NOT_FOUND);
    let system = h
        .llm
        .requests
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .system
        .clone();
    assert!(system.contains("```html"));
}
