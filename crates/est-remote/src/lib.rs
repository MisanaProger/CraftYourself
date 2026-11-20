//! Code forges and online judges over HTTPS, behind
//! [`est_application::ports::RemoteSources`].
//!
//! * GitHub REST API — Issue, repository, comments;
//! * GitLab REST API v4 (gitlab.com and configured self-hosted instances);
//! * Codeforces — `contest.standings` for the title, rating and tags, the
//!   problem page for the statement and samples;
//! * LeetCode — the public GraphQL `question` query.
//!
//! The application calls this adapter only after the policy's gate allowed
//! the action (`est_ai_policy::gates`). Tokens are read from environment
//! variables named in the configuration, used only for publishing (and to
//! raise GitHub's rate limit), and never logged or returned.

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use est_application::ports::{FetchedProblem, RemoteSources};
use est_application::{AppError, AppResult};
use est_issues::{Forge, IssueRef, RemoteIssue, RemoteRepository};
use est_problems::{Platform, ProblemInput, ProblemRef};
use serde::Deserialize;
use serde_json::{Value, json};

/// Largest response EST reads.
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// `[remote]` in `config.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RemoteConfig {
    /// Self-hosted GitLab instances, e.g. `git.example.org`.
    pub gitlab_hosts: Vec<String>,
    /// Environment variable with a GitHub token (publishing, rate limits).
    pub github_token_env: Option<String>,
    /// Environment variable with a GitLab token (publishing).
    pub gitlab_token_env: Option<String>,
    pub timeout_secs: u64,
    /// API endpoints; changed only by tests.
    #[serde(skip)]
    pub endpoints: Endpoints,
}

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub github_api: String,
    /// `None` = `https://<host>/api/v4`.
    pub gitlab_api: Option<String>,
    pub codeforces: String,
    pub leetcode: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            github_api: "https://api.github.com".into(),
            gitlab_api: None,
            codeforces: "https://codeforces.com".into(),
            leetcode: "https://leetcode.com".into(),
        }
    }
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            gitlab_hosts: Vec::new(),
            github_token_env: Some("GITHUB_TOKEN".into()),
            gitlab_token_env: Some("GITLAB_TOKEN".into()),
            timeout_secs: 20,
            endpoints: Endpoints::default(),
        }
    }
}

pub struct HttpRemote {
    config: RemoteConfig,
    client: reqwest::Client,
}

fn infra(e: impl std::fmt::Display) -> AppError {
    AppError::Infrastructure(e.to_string())
}

fn token(var: &Option<String>) -> Option<String> {
    var.as_deref()
        .and_then(|v| std::env::var(v).ok())
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

fn time(v: &Value) -> Option<DateTime<Utc>> {
    v.as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&Utc))
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

impl HttpRemote {
    pub fn new(config: RemoteConfig) -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs.max(1)))
            .user_agent(concat!("CraftYourself/", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()
            .map_err(infra)?;
        Ok(Self { config, client })
    }

    fn gitlab_api(&self, host: &str) -> String {
        self.config
            .endpoints
            .gitlab_api
            .clone()
            .unwrap_or_else(|| format!("https://{host}/api/v4"))
    }

    fn github_token(&self) -> Option<String> {
        token(&self.config.github_token_env)
    }

    fn gitlab_token(&self) -> Option<String> {
        token(&self.config.gitlab_token_env)
    }

    async fn read(&self, req: reqwest::RequestBuilder, what: &str) -> AppResult<String> {
        let res = req
            .send()
            .await
            .map_err(|e| infra(format!("{what}: {}", e.without_url())))?;
        let status = res.status();
        if res
            .content_length()
            .is_some_and(|n| n as usize > MAX_RESPONSE_BYTES)
        {
            return Err(infra(format!("{what}: the response is too large")));
        }
        let body = res.bytes().await.map_err(|e| infra(e.without_url()))?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(infra(format!("{what}: the response is too large")));
        }
        match status.as_u16() {
            200..=299 => Ok(String::from_utf8_lossy(&body).into_owned()),
            404 => Err(AppError::NotFound(what.into())),
            401 | 403 | 429 => Err(AppError::Invalid(format!(
                "{what}: access refused ({status}); the item may be private or the rate limit reached"
            ))),
            _ => Err(infra(format!("{what}: {status}"))),
        }
    }

    async fn json(&self, req: reqwest::RequestBuilder, what: &str) -> AppResult<Value> {
        let text = self.read(req, what).await?;
        serde_json::from_str(&text).map_err(|e| infra(format!("{what}: {e}")))
    }

    fn github(&self, path: &str) -> reqwest::RequestBuilder {
        let mut req = self
            .client
            .get(format!("{}{path}", self.config.endpoints.github_api))
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", "2022-11-28");
        if let Some(t) = self.github_token() {
            req = req.bearer_auth(t);
        }
        req
    }

    async fn github_issue(&self, r: &IssueRef) -> AppResult<RemoteIssue> {
        let issue = self
            .json(
                self.github(&format!("/repos/{}/issues/{}", r.project, r.number)),
                "issue",
            )
            .await?;
        let repo = self
            .json(self.github(&format!("/repos/{}", r.project)), "repository")
            .await?;
        Ok(RemoteIssue {
            title: text(&issue["title"]),
            body: text(&issue["body"]),
            open: issue["state"] == "open",
            labels: issue["labels"]
                .as_array()
                .map(|a| a.iter().map(|l| text(&l["name"])).collect())
                .unwrap_or_default(),
            assignees: issue["assignees"].as_array().map_or(0, |a| a.len() as u32),
            comments: issue["comments"].as_u64().unwrap_or(0) as u32,
            pull_request: issue.get("pull_request").is_some_and(|p| !p.is_null()),
            locked: issue["locked"].as_bool().unwrap_or(false),
            created_at: time(&issue["created_at"]),
            updated_at: time(&issue["updated_at"]),
            repository: RemoteRepository {
                clone_url: repo["clone_url"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| r.clone_url()),
                default_branch: repo["default_branch"].as_str().map(str::to_owned),
                archived: repo["archived"].as_bool().unwrap_or(false),
                license: repo["license"]["spdx_id"]
                    .as_str()
                    .filter(|l| *l != "NOASSERTION")
                    .or(repo["license"]["name"].as_str())
                    .map(str::to_owned),
                language: repo["language"].as_str().map(str::to_owned),
            },
        })
    }

    fn gitlab(&self, host: &str, path: &str) -> reqwest::RequestBuilder {
        self.client.get(format!("{}{path}", self.gitlab_api(host)))
    }

    async fn gitlab_issue(&self, r: &IssueRef) -> AppResult<RemoteIssue> {
        let id = url::form_urlencoded::byte_serialize(r.project.as_bytes()).collect::<String>();
        let issue = self
            .json(
                self.gitlab(&r.host, &format!("/projects/{id}/issues/{}", r.number)),
                "issue",
            )
            .await?;
        let repo = self
            .json(
                self.gitlab(&r.host, &format!("/projects/{id}?license=true")),
                "repository",
            )
            .await?;
        Ok(RemoteIssue {
            title: text(&issue["title"]),
            body: text(&issue["description"]),
            open: issue["state"] == "opened",
            labels: issue["labels"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|l| {
                            l.as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| text(&l["name"]))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            assignees: issue["assignees"].as_array().map_or(0, |a| a.len() as u32),
            comments: issue["user_notes_count"].as_u64().unwrap_or(0) as u32,
            pull_request: false,
            locked: issue["discussion_locked"].as_bool().unwrap_or(false),
            created_at: time(&issue["created_at"]),
            updated_at: time(&issue["updated_at"]),
            repository: RemoteRepository {
                clone_url: repo["http_url_to_repo"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| r.clone_url()),
                default_branch: repo["default_branch"].as_str().map(str::to_owned),
                archived: repo["archived"].as_bool().unwrap_or(false),
                license: repo["license"]["nickname"]
                    .as_str()
                    .or(repo["license"]["key"].as_str())
                    .or(repo["license"]["name"].as_str())
                    .map(str::to_owned),
                language: None,
            },
        })
    }

    async fn codeforces(&self, p: &ProblemRef) -> AppResult<FetchedProblem> {
        let split =
            p.id.find(|c: char| !c.is_ascii_digit())
                .unwrap_or(p.id.len());
        let (contest, index) = p.id.split_at(split);
        let base = &self.config.endpoints.codeforces;
        // Title, rating and tags; gym contests may need a login.
        let meta = self
            .json(
                self.client.get(format!(
                    "{base}/api/contest.standings?contestId={contest}&from=1&count=1"
                )),
                "problem",
            )
            .await
            .ok()
            .and_then(|v| {
                v["result"]["problems"]
                    .as_array()?
                    .iter()
                    .find(|x| x["index"].as_str() == Some(index))
                    .cloned()
            });
        let path = url::Url::parse(&p.url)
            .map(|u| u.path().to_owned())
            .unwrap_or_default();
        let page = self
            .read(self.client.get(format!("{base}{path}")), "problem page")
            .await;
        let statement = page
            .as_deref()
            .ok()
            .and_then(est_problems::html::codeforces_statement);
        if meta.is_none() && statement.is_none() {
            return Err(match page {
                Err(e @ AppError::NotFound(_)) => e,
                _ => AppError::Invalid(
                    "Codeforces did not return the problem; paste the statement".into(),
                ),
            });
        }
        let meta = meta.unwrap_or(Value::Null);
        let statement_found = statement.is_some();
        let (title, markdown, samples) = match statement {
            Some(s) => (s.title, s.markdown, s.samples),
            None => (None, String::new(), Vec::new()),
        };
        Ok(FetchedProblem {
            input: ProblemInput {
                url: Some(p.url.clone()),
                title: meta["name"].as_str().map(str::to_owned).or(title),
                statement: markdown,
                samples,
                tags: meta["tags"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| t.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
                rating: meta["rating"].as_u64().map(|r| r as u32),
                difficulty: None,
            },
            statement_found,
        })
    }

    async fn leetcode(&self, p: &ProblemRef) -> AppResult<FetchedProblem> {
        let base = &self.config.endpoints.leetcode;
        let query = "query q($titleSlug: String!) { question(titleSlug: $titleSlug) { title content difficulty isPaidOnly topicTags { name } } }";
        let v = self
            .json(
                self.client
                    .post(format!("{base}/graphql"))
                    .header("referer", format!("{base}/problems/{}/", p.id))
                    .json(&json!({ "query": query, "variables": { "titleSlug": p.id } })),
                "problem",
            )
            .await?;
        let q = &v["data"]["question"];
        if q.is_null() {
            return Err(AppError::NotFound("problem".into()));
        }
        let content = q["content"].as_str().unwrap_or_default();
        if content.trim().is_empty() {
            return Err(AppError::Invalid(if q["isPaidOnly"] == true {
                "this is a LeetCode Premium problem; paste the statement".into()
            } else {
                "LeetCode returned no statement; paste it".into()
            }));
        }
        let s = est_problems::html::leetcode_statement(content);
        Ok(FetchedProblem {
            input: ProblemInput {
                url: Some(p.url.clone()),
                title: q["title"].as_str().map(str::to_owned),
                statement: s.markdown,
                samples: s.samples,
                tags: q["topicTags"]
                    .as_array()
                    .map(|a| a.iter().map(|t| text(&t["name"])).collect())
                    .unwrap_or_default(),
                rating: None,
                difficulty: q["difficulty"].as_str().map(str::to_ascii_lowercase),
            },
            statement_found: true,
        })
    }
}

#[async_trait]
impl RemoteSources for HttpRemote {
    async fn issue(&self, r: &IssueRef) -> AppResult<RemoteIssue> {
        match r.forge {
            Forge::Github => self.github_issue(r).await,
            Forge::Gitlab => self.gitlab_issue(r).await,
        }
    }

    async fn problem(&self, p: &ProblemRef) -> AppResult<FetchedProblem> {
        match p.platform {
            Platform::Codeforces => self.codeforces(p).await,
            Platform::Leetcode if p.url.starts_with("https://leetcode.com/") => {
                self.leetcode(p).await
            }
            _ => Err(AppError::Invalid(
                "statements are loaded from codeforces.com and leetcode.com only; paste it".into(),
            )),
        }
    }

    fn can_publish(&self, r: &IssueRef) -> bool {
        match r.forge {
            Forge::Github => self.github_token().is_some(),
            Forge::Gitlab => self.gitlab_token().is_some(),
        }
    }

    async fn comment(&self, r: &IssueRef, body: &str) -> AppResult<String> {
        let missing = || AppError::Invalid("no token for publishing is configured".into());
        match r.forge {
            Forge::Github => {
                let t = self.github_token().ok_or_else(missing)?;
                let v = self
                    .json(
                        self.client
                            .post(format!(
                                "{}/repos/{}/issues/{}/comments",
                                self.config.endpoints.github_api, r.project, r.number
                            ))
                            .bearer_auth(t)
                            .header("accept", "application/vnd.github+json")
                            .header("x-github-api-version", "2022-11-28")
                            .json(&json!({ "body": body })),
                        "comment",
                    )
                    .await?;
                Ok(v["html_url"].as_str().unwrap_or(&r.url).to_owned())
            }
            Forge::Gitlab => {
                let t = self.gitlab_token().ok_or_else(missing)?;
                let id =
                    url::form_urlencoded::byte_serialize(r.project.as_bytes()).collect::<String>();
                let v = self
                    .json(
                        self.client
                            .post(format!(
                                "{}/projects/{id}/issues/{}/notes",
                                self.gitlab_api(&r.host),
                                r.number
                            ))
                            .header("private-token", t)
                            .json(&json!({ "body": body })),
                        "comment",
                    )
                    .await?;
                Ok(match v["id"].as_u64() {
                    Some(n) => format!("{}#note_{n}", r.url),
                    None => r.url.clone(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path, Query};
    use axum::http::HeaderMap;
    use axum::routing::{get, post};
    use axum::{Json, Router};

    async fn serve(app: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    fn remote(base: &str, token_env: Option<&str>) -> HttpRemote {
        HttpRemote::new(RemoteConfig {
            github_token_env: token_env.map(str::to_owned),
            gitlab_token_env: token_env.map(str::to_owned),
            endpoints: Endpoints {
                github_api: base.into(),
                gitlab_api: Some(format!("{base}/api/v4")),
                codeforces: base.into(),
                leetcode: base.into(),
            },
            ..RemoteConfig::default()
        })
        .unwrap()
    }

    #[tokio::test]
    async fn reads_github_issues_and_publishes_comments() {
        let app = Router::new()
            .route(
                "/repos/o/r/issues/5",
                get(|| async {
                    Json(json!({
                        "title": "Panic on empty input", "body": "Steps\n- [ ] fix", "state": "open",
                        "labels": [{ "name": "bug" }], "assignees": [], "comments": 3, "locked": false,
                        "created_at": "2026-01-02T03:04:05Z", "updated_at": "2026-02-02T03:04:05Z"
                    }))
                }),
            )
            .route(
                "/repos/o/r",
                get(|| async {
                    Json(json!({
                        "clone_url": "https://github.com/o/r.git", "default_branch": "main",
                        "archived": false, "license": { "spdx_id": "MIT" }, "language": "Rust"
                    }))
                }),
            )
            .route(
                "/repos/o/r/issues/5/comments",
                post(|headers: HeaderMap, Json(body): Json<Value>| async move {
                    assert_eq!(headers["authorization"], "Bearer test-token");
                    assert_eq!(body["body"], "On it");
                    Json(json!({ "html_url": "https://github.com/o/r/issues/5#issuecomment-1" }))
                }),
            );
        let base = serve(app).await;
        let r = est_issues::parse_issue_url("https://github.com/o/r/issues/5", &[]).unwrap();
        let issue = remote(&base, None).issue(&r).await.unwrap();
        assert_eq!(issue.title, "Panic on empty input");
        assert!(issue.open && !issue.pull_request);
        assert_eq!(issue.labels, ["bug"]);
        assert_eq!(issue.repository.license.as_deref(), Some("MIT"));
        assert_eq!(
            issue.updated_at.unwrap().to_rfc3339(),
            "2026-02-02T03:04:05+00:00"
        );

        let missing = remote(&base, Some("EST_REMOTE_TEST_NO_SUCH_VAR"));
        assert!(!missing.can_publish(&r));
        assert!(missing.comment(&r, "On it").await.is_err());
        // SAFETY: only this test reads the variable.
        unsafe { std::env::set_var("EST_REMOTE_TEST_TOKEN", "test-token") };
        let with = remote(&base, Some("EST_REMOTE_TEST_TOKEN"));
        assert!(with.can_publish(&r));
        assert_eq!(
            with.comment(&r, "On it").await.unwrap(),
            "https://github.com/o/r/issues/5#issuecomment-1"
        );
        let other = est_issues::parse_issue_url("https://github.com/o/r/issues/6", &[]).unwrap();
        assert!(matches!(
            remote(&base, None).issue(&other).await,
            Err(AppError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn reads_gitlab_issues() {
        let app = Router::new()
            .route(
                "/api/v4/projects/{id}/issues/7",
                get(|Path(id): Path<String>| async move {
                    assert_eq!(id, "g/sub/app");
                    Json(json!({
                        "title": "Docs typo", "description": "Fix it", "state": "opened",
                        "labels": ["documentation"], "assignees": [{}], "user_notes_count": 1
                    }))
                }),
            )
            .route(
                "/api/v4/projects/{id}",
                get(|| async {
                    Json(json!({ "http_url_to_repo": "https://gitlab.com/g/sub/app.git", "archived": true }))
                }),
            );
        let base = serve(app).await;
        let r =
            est_issues::parse_issue_url("https://gitlab.com/g/sub/app/-/issues/7", &[]).unwrap();
        let issue = remote(&base, None).issue(&r).await.unwrap();
        assert_eq!((issue.open, issue.assignees), (true, 1));
        assert!(issue.repository.archived);
        assert_eq!(issue.labels, ["documentation"]);
    }

    #[tokio::test]
    async fn reads_judges() {
        #[derive(Deserialize)]
        struct Q {
            #[serde(rename = "contestId")]
            contest_id: u32,
        }
        let app = Router::new()
            .route(
                "/api/contest.standings",
                get(|Query(q): Query<Q>| async move {
                    assert_eq!(q.contest_id, 4);
                    Json(json!({ "status": "OK", "result": { "problems": [
                        { "index": "A", "name": "Watermelon", "rating": 800, "tags": ["brute force", "math"] }
                    ]}}))
                }),
            )
            .route(
                "/problemset/problem/4/A",
                get(|| async {
                    axum::response::Html(
                        "<div class=\"problem-statement\"><div class=\"header\"><div class=\"title\">A. Watermelon</div></div><div><p>Pete and Billy bought a watermelon weighing $$$w$$$ kilos.</p></div><div class=\"sample-tests\"><div class=\"sample-test\"><div class=\"input\"><pre>8</pre></div><div class=\"output\"><pre>YES</pre></div></div></div></div>",
                    )
                }),
            )
            .route(
                "/graphql",
                post(|Json(body): Json<Value>| async move {
                    let slug = body["variables"]["titleSlug"].as_str().unwrap().to_owned();
                    Json(match slug.as_str() {
                        "two-sum" => json!({ "data": { "question": {
                            "title": "Two Sum", "difficulty": "Easy", "isPaidOnly": false,
                            "topicTags": [{ "name": "Array" }, { "name": "Hash Table" }],
                            "content": "<p>Return indices.</p><pre><strong>Input:</strong> nums = [2,7], target = 9\n<strong>Output:</strong> [0,1]</pre>"
                        }}}),
                        "premium" => json!({ "data": { "question": { "title": "P", "content": null, "isPaidOnly": true } } }),
                        _ => json!({ "data": { "question": null } }),
                    })
                }),
            );
        let base = serve(app).await;
        let r = remote(&base, None);
        let cf = est_problems::parse_url("https://codeforces.com/problemset/problem/4/A").unwrap();
        let p = r.problem(&cf).await.unwrap();
        assert!(p.statement_found);
        assert_eq!(p.input.title.as_deref(), Some("Watermelon"));
        assert_eq!((p.input.rating, p.input.tags.len()), (Some(800), 2));
        assert_eq!(p.input.samples[0].output, "YES");
        // The result is what the learner would paste.
        let normalized = est_problems::normalize(&p.input).unwrap();
        assert!(normalized.statement.contains("$w$ kilos"));

        let lc = est_problems::parse_url("https://leetcode.com/problems/two-sum/").unwrap();
        let p = r.problem(&lc).await.unwrap();
        assert_eq!(p.input.difficulty.as_deref(), Some("easy"));
        assert_eq!(p.input.tags, ["Array", "Hash Table"]);
        assert_eq!(p.input.samples[0].input, "nums = [2,7], target = 9");
        let premium = est_problems::parse_url("https://leetcode.com/problems/premium/").unwrap();
        assert!(matches!(
            r.problem(&premium).await,
            Err(AppError::Invalid(_))
        ));
        let missing = est_problems::parse_url("https://leetcode.com/problems/nope/").unwrap();
        assert!(matches!(
            r.problem(&missing).await,
            Err(AppError::NotFound(_))
        ));
        let other = est_problems::parse_url("https://atcoder.jp/contests/abc1/tasks/a").unwrap();
        assert!(matches!(r.problem(&other).await, Err(AppError::Invalid(_))));
    }
}
