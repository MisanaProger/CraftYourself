//! Sandbox Manager (architecture v0.3, §17).
//!
//! Commands from open-source repositories are untrusted, so they never run
//! with the daemon's privileges. Every run is a fresh container:
//!
//! * the workspace is mounted **read-only** at `/src` and copied into a
//!   tmpfs at `/work` — code under test cannot modify the learner's files;
//! * the root filesystem is read-only; only `/work`, `/tmp` and two cache
//!   volumes (build cache per session, dependency cache per project) are
//!   writable;
//! * no network unless the learner explicitly allowed it for this run;
//! * CPU, memory, process-count and wall-clock limits;
//! * all capabilities dropped, `no-new-privileges`, never `--privileged`;
//! * no host environment variables or secrets are passed in;
//! * the container is removed when the run ends (`--rm`).
//!
//! Docker and Podman share the CLI used here; rootless Podman is preferred
//! when both are installed.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use est_application::ports::{Sandbox, SandboxInfo, SandboxOutcome, SandboxRequest};
use est_application::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxConfig {
    /// `auto` (Podman, then Docker), `podman`, `docker`, or a path.
    pub runtime: String,
    /// Image per language adapter id.
    pub images: BTreeMap<String, String>,
    /// Image when the language is unknown.
    pub default_image: String,
    pub cpus: f32,
    pub memory_mb: u32,
    pub pids_limit: u32,
    pub timeout_secs: u64,
    /// Size of the tmpfs the workspace is copied into.
    pub work_mb: u32,
    /// Captured output per stream (head and tail are kept).
    pub output_kb: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        let images = [
            ("rust", "docker.io/library/rust:1-slim"),
            ("c", "docker.io/library/gcc:14"),
            ("cpp", "docker.io/library/gcc:14"),
            ("python", "docker.io/library/python:3-slim"),
            ("javascript", "docker.io/library/node:22-slim"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        Self {
            runtime: "auto".into(),
            images,
            default_image: "docker.io/library/debian:stable-slim".into(),
            cpus: 2.0,
            memory_mb: 2048,
            pids_limit: 512,
            timeout_secs: 600,
            work_mb: 2048,
            output_kb: 128,
        }
    }
}

/// Docker/Podman implementation of the [`Sandbox`] port.
pub struct ContainerSandbox {
    config: SandboxConfig,
    runtime: Option<String>,
    reason: Option<String>,
}

fn in_path(program: &str) -> bool {
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join(program).is_file()))
}

/// Container, volume and similar names: lowercase alphanumerics and dashes.
fn sanitize(key: &str) -> String {
    let s: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    s.trim_matches('-').chars().take(60).collect()
}

fn valid_workdir(workdir: &str) -> bool {
    let p = Path::new(workdir);
    !p.is_absolute()
        && p.components().all(|c| {
            matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
        && !workdir.contains(['\n', '\0'])
}

/// The script run inside the container. Values come from environment
/// variables, never from string interpolation.
const ENTRYPOINT: &str = r#"set -e
tar -C /src -cf - . | tar -C /work -xf - --no-same-owner
cd "/work/$EST_WORKDIR"
exec sh -c "$EST_COMMAND""#;

impl ContainerSandbox {
    /// Picks a runtime and checks that it can reach its engine.
    pub fn detect(config: SandboxConfig) -> Self {
        let candidates: Vec<String> = match config.runtime.as_str() {
            "auto" => vec!["podman".into(), "docker".into()],
            other => vec![other.to_owned()],
        };
        let mut reason = Some(format!(
            "no container runtime found ({}); install Podman or Docker",
            candidates.join(", ")
        ));
        for bin in candidates {
            if !in_path(&bin) {
                continue;
            }
            let ok = std::process::Command::new(&bin)
                .args(["info", "--format", "{{.ServerVersion}}"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if ok {
                return Self {
                    config,
                    runtime: Some(bin),
                    reason: None,
                };
            }
            reason = Some(format!(
                "`{bin}` is installed but its engine is not reachable (is the daemon/service running?)"
            ));
        }
        Self {
            config,
            runtime: None,
            reason,
        }
    }

    /// A sandbox bound to a specific runtime binary without probing it (tests).
    pub fn with_runtime(config: SandboxConfig, runtime: impl Into<String>) -> Self {
        Self {
            config,
            runtime: Some(runtime.into()),
            reason: None,
        }
    }

    pub fn image_for(&self, language: Option<&str>) -> String {
        language
            .and_then(|l| self.config.images.get(l))
            .cloned()
            .unwrap_or_else(|| self.config.default_image.clone())
    }

    fn container_name(run_id: &str) -> String {
        format!("est-run-{}", sanitize(run_id))
    }

    /// Full argument list for `<runtime> run …`.
    pub fn run_args(&self, req: &SandboxRequest, image: &str) -> AppResult<Vec<String>> {
        if !valid_workdir(&req.workdir) {
            return Err(AppError::Invalid(format!(
                "invalid working directory `{}`",
                req.workdir
            )));
        }
        let workspace = req.workspace.canonicalize().map_err(|e| {
            AppError::Invalid(format!("workspace {}: {e}", req.workspace.display()))
        })?;
        let workspace = workspace
            .to_str()
            .filter(|s| !s.contains(','))
            .ok_or_else(|| {
                AppError::Invalid("workspace path must be UTF-8 without commas".into())
            })?;
        let c = &self.config;
        let mut a: Vec<String> = Vec::new();
        let mut push = |xs: &[&str]| a.extend(xs.iter().map(|s| (*s).to_owned()));
        push(&["run", "--rm", "--name", &Self::container_name(&req.run_id)]);
        push(&["--network", if req.network { "bridge" } else { "none" }]);
        push(&["--cpus", &format!("{}", c.cpus)]);
        push(&[
            "--memory",
            &format!("{}m", c.memory_mb),
            "--memory-swap",
            &format!("{}m", c.memory_mb),
        ]);
        push(&["--pids-limit", &c.pids_limit.to_string()]);
        push(&["--cap-drop", "ALL", "--security-opt", "no-new-privileges"]);
        push(&["--read-only"]);
        push(&["--tmpfs", "/tmp:rw,exec,size=512m"]);
        push(&["--tmpfs", &format!("/work:rw,exec,size={}m", c.work_mb)]);
        push(&[
            "--mount",
            &format!("type=bind,source={workspace},target=/src,readonly"),
        ]);
        push(&[
            "--mount",
            &format!(
                "type=volume,source=est-cache-{},target=/cache",
                sanitize(&req.cache_key)
            ),
        ]);
        push(&[
            "--mount",
            &format!(
                "type=volume,source=est-deps-{},target=/deps",
                sanitize(&req.deps_key)
            ),
        ]);
        for (k, v) in [
            ("HOME", "/tmp"),
            ("CI", "1"),
            ("CARGO_HOME", "/deps/cargo"),
            ("CARGO_TARGET_DIR", "/cache/target"),
            ("CARGO_TERM_COLOR", "never"),
            ("PIP_CACHE_DIR", "/deps/pip"),
            ("PYTHONPATH", "/deps/python"),
            ("PYTHONDONTWRITEBYTECODE", "1"),
            ("npm_config_cache", "/deps/npm"),
            ("EST_WORKDIR", &req.workdir),
            ("EST_COMMAND", &req.command),
        ] {
            push(&["--env", &format!("{k}={v}")]);
        }
        push(&[
            "--workdir",
            "/work",
            "--entrypoint",
            "sh",
            image,
            "-c",
            ENTRYPOINT,
        ]);
        Ok(a)
    }
}

/// Keeps the first `head` and the last `tail` bytes of a stream.
async fn capture<R: AsyncRead + Unpin>(mut r: R, limit: usize) -> (String, bool) {
    let head_cap = limit / 4;
    let tail_cap = limit - head_cap;
    let mut head = Vec::new();
    let mut tail: std::collections::VecDeque<u8> = std::collections::VecDeque::new();
    let mut truncated = false;
    let mut buf = [0u8; 8192];
    loop {
        let n = match r.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for &b in &buf[..n] {
            if head.len() < head_cap {
                head.push(b);
            } else {
                tail.push_back(b);
                if tail.len() > tail_cap {
                    tail.pop_front();
                    truncated = true;
                }
            }
        }
    }
    let mut out = String::from_utf8_lossy(&head).into_owned();
    if truncated {
        out.push_str("\n… [output truncated] …\n");
    }
    out.push_str(&String::from_utf8_lossy(tail.make_contiguous()));
    (out, truncated)
}

#[async_trait]
impl Sandbox for ContainerSandbox {
    fn info(&self) -> SandboxInfo {
        let c = &self.config;
        SandboxInfo {
            runtime: self.runtime.clone(),
            available: self.runtime.is_some(),
            reason: self.reason.clone(),
            limits: serde_json::json!({
                "cpus": c.cpus,
                "memory_mb": c.memory_mb,
                "pids_limit": c.pids_limit,
                "timeout_secs": c.timeout_secs,
                "network": "off unless allowed per run",
            }),
        }
    }

    async fn run(&self, req: SandboxRequest) -> AppResult<SandboxOutcome> {
        let runtime = self.runtime.clone().ok_or_else(|| {
            AppError::Infrastructure(
                self.reason
                    .clone()
                    .unwrap_or_else(|| "sandbox unavailable".into()),
            )
        })?;
        let image = self.image_for(req.language.as_deref());
        let args = self.run_args(&req, &image)?;
        let started = Instant::now();
        let mut child = Command::new(&runtime)
            .args(&args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .envs(std::env::var_os("XDG_RUNTIME_DIR").map(|v| ("XDG_RUNTIME_DIR", v)))
            .envs(std::env::var_os("HOME").map(|v| ("HOME", v)))
            .envs(std::env::var_os("DOCKER_HOST").map(|v| ("DOCKER_HOST", v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AppError::Infrastructure(format!("cannot start {runtime}: {e}")))?;
        let limit = self.config.output_kb * 1024;
        let out = tokio::spawn(capture(child.stdout.take().expect("piped"), limit));
        let err = tokio::spawn(capture(child.stderr.take().expect("piped"), limit));

        let timeout = Duration::from_secs(self.config.timeout_secs);
        let (status, timed_out) = match tokio::time::timeout(timeout, child.wait()).await {
            Ok(status) => (status.ok(), false),
            Err(_) => {
                let name = Self::container_name(&req.run_id);
                let _ = Command::new(&runtime).args(["kill", &name]).output().await;
                let _ = child.kill().await;
                (child.wait().await.ok(), true)
            }
        };
        let (stdout, _) = out.await.unwrap_or_default();
        let (stderr, _) = err.await.unwrap_or_default();
        Ok(SandboxOutcome {
            exit_code: if timed_out {
                None
            } else {
                status.and_then(|s| s.code())
            },
            stdout,
            stderr,
            duration_ms: started.elapsed().as_millis() as u64,
            timed_out,
            image,
        })
    }

    async fn discard(&self, cache_key: &str) -> AppResult<()> {
        if let Some(runtime) = &self.runtime {
            let name = format!("est-cache-{}", sanitize(cache_key));
            let _ = Command::new(runtime)
                .args(["volume", "rm", "-f", &name])
                .output()
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(dir: &Path, workdir: &str, network: bool) -> SandboxRequest {
        SandboxRequest {
            run_id: "Run_1".into(),
            workspace: dir.to_owned(),
            workdir: workdir.into(),
            command: "cargo test; echo $HOME".into(),
            language: Some("rust".into()),
            network,
            cache_key: "session-1".into(),
            deps_key: "project/1".into(),
        }
    }

    #[test]
    fn isolation_flags() {
        let dir = tempfile::tempdir().unwrap();
        let sb = ContainerSandbox::with_runtime(SandboxConfig::default(), "docker");
        let image = sb.image_for(Some("rust"));
        assert!(image.contains("rust"));
        assert_eq!(
            sb.image_for(Some("cobol")),
            SandboxConfig::default().default_image
        );
        let args = sb.run_args(&req(dir.path(), "", false), &image).unwrap();
        let joined = args.join(" ");
        for flag in [
            "--rm",
            "--network none",
            "--cap-drop ALL",
            "--security-opt no-new-privileges",
            "--read-only",
            "--pids-limit 512",
            "--memory 2048m",
            "--name est-run-run-1",
            "type=volume,source=est-deps-project-1,target=/deps",
        ] {
            assert!(joined.contains(flag), "missing {flag} in {joined}");
        }
        assert!(joined.contains(&format!(
            "source={},target=/src,readonly",
            dir.path().canonicalize().unwrap().display()
        )));
        assert!(!joined.contains("--privileged"));
        // The command travels as data, not as part of the script.
        assert!(args.contains(&"EST_COMMAND=cargo test; echo $HOME".to_owned()));
        assert_eq!(args.last().unwrap(), ENTRYPOINT);

        let net = sb
            .run_args(&req(dir.path(), "crates/a", true), &image)
            .unwrap()
            .join(" ");
        assert!(net.contains("--network bridge"));
    }

    #[test]
    fn rejects_escaping_workdirs() {
        let dir = tempfile::tempdir().unwrap();
        let sb = ContainerSandbox::with_runtime(SandboxConfig::default(), "docker");
        for bad in ["../x", "/etc", "a/../../b"] {
            assert!(
                sb.run_args(&req(dir.path(), bad, false), "img").is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn unavailable_runtime_is_reported() {
        let sb = ContainerSandbox::detect(SandboxConfig {
            runtime: "definitely-not-a-runtime".into(),
            ..Default::default()
        });
        let info = sb.info();
        assert!(!info.available);
        assert!(info.reason.unwrap().contains("no container runtime"));
    }

    #[tokio::test]
    async fn capture_keeps_head_and_tail() {
        let data: Vec<u8> = (0..10_000).map(|i| b'a' + (i % 26) as u8).collect();
        let (s, truncated) = capture(&data[..], 1000).await;
        assert!(truncated);
        assert!(s.starts_with("abcdef"));
        assert!(s.contains("[output truncated]"));
        let tail: String = data[data.len() - 750..]
            .iter()
            .map(|&b| b as char)
            .collect();
        assert!(s.ends_with(&tail));
    }

    /// Runs real containers; enable with `EST_SANDBOX_IT=1`.
    #[tokio::test]
    async fn real_container_isolation() {
        if std::env::var("EST_SANDBOX_IT").as_deref() != Ok("1") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "hi").unwrap();
        let sb = ContainerSandbox::detect(SandboxConfig {
            timeout_secs: 20,
            ..Default::default()
        });
        assert!(sb.info().available, "{:?}", sb.info().reason);
        let mut r = req(dir.path(), "", false);
        r.command = "cat hello.txt; echo changed > hello.txt; cat hello.txt; \
                     (getent hosts example.com >/dev/null && echo NET) || echo NONET; touch /etc/x 2>/dev/null || echo ROFS"
            .into();
        let out = sb.run(r.clone()).await.unwrap();
        assert_eq!(out.exit_code, Some(0), "{out:?}");
        assert!(out.stdout.contains("hichanged"), "{out:?}");
        assert!(
            out.stdout.contains("NONET") && out.stdout.contains("ROFS"),
            "{out:?}"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(),
            "hi",
            "host files untouched"
        );

        r.run_id = "timeout".into();
        r.command = "sleep 60".into();
        let sb = ContainerSandbox::detect(SandboxConfig {
            timeout_secs: 2,
            ..Default::default()
        });
        let out = sb.run(r).await.unwrap();
        assert!(out.timed_out);
        sb.discard("session-1").await.unwrap();
    }
}
