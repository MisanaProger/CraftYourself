import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Icon } from "../app/icons";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { LearningSession, RunRequest, SandboxRun, Task } from "../types/api";
import { Empty, ErrorBox, tv, useFormat } from "../ui";

function Run({ run }: { run: SandboxRun }) {
  const { t } = useT();
  const fmt = useFormat();
  const output = [run.stdout, run.stderr, run.error ?? ""].filter(Boolean).join("\n").trim();
  return (
    <li className={`run run-${run.status}`}>
      <details>
        <summary>
          <span className="run-dot" aria-hidden />
          <strong>{tv(t, "runStatus", run.status)}</strong>
          <code>{run.command}</code>
          <span className="muted small push">
            {run.summary && t("tests.short", { passed: run.summary.passed, failed: run.summary.failed })}
            {run.duration_ms != null && ` · ${fmt.duration(run.duration_ms)}`}
          </span>
        </summary>
        <p className="muted small">
          {tv(t, "runKind", run.kind)} · {fmt.time(run.started_at)}
          {run.network && ` · ${t("tests.withNetwork")}`}
          {run.image && ` · ${run.image}`}
        </p>
        {output ? <pre className="output">{output}</pre> : <Empty>{run.status === "running" ? t("tests.running") : t("tests.noOutput")}</Empty>}
      </details>
    </li>
  );
}

export function TestsPanel({ task, session }: { task: Task | null; session: LearningSession | null }) {
  const { t } = useT();
  const qc = useQueryClient();
  const [custom, setCustom] = useState("");
  const info = useQuery({ queryKey: ["sandbox"], queryFn: api.sandbox });
  const runs = useQuery({
    queryKey: ["session", session?.id, "runs"],
    queryFn: () => api.runs(session!.id),
    enabled: !!session,
    refetchInterval: (q) => (q.state.data?.some((r) => r.status === "running") ? 1500 : false),
  });
  const commands = useQuery({ queryKey: ["projects", task?.project_id, "commands"], queryFn: () => api.projectCommands(task!.project_id), enabled: !!task });
  const start = useMutation({
    mutationFn: (req: RunRequest) => api.startRun(session!.id, req),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["session", session?.id, "runs"] }),
  });
  if (!task || !session) return <Empty>{t("tests.noSession")}</Empty>;
  const active = session.status === "active";
  const running = runs.data?.some((r) => r.status === "running");
  const available = info.data?.available;
  const testCommand = task.test_command ?? commands.data?.find((c) => c.kind === "test") ?? null;
  const disabled = !active || !available || running || start.isPending;

  return (
    <div className="tests">
      <div className={`sandbox-status ${available ? "on" : "off"}`}>
        <i aria-hidden />
        {info.data
          ? available
            ? t("tests.sandboxOn", { runtime: info.data.runtime ?? "" })
            : t("tests.sandboxOff", { reason: info.data.reason ?? "" })
          : t("common.loading")}
      </div>
      <p className="muted small">{t("tests.isolation")}</p>

      <div className="stack">
        <button className="primary run-btn" disabled={disabled || !testCommand} onClick={() => start.mutate({ kind: "test" })}>
          <Icon.play /> {running ? t("tests.running") : t("tests.run")}
        </button>
        {testCommand && (
          <p className="muted small">
            {t("tests.command")}: <code>{typeof testCommand === "string" ? testCommand : [testCommand.program, ...testCommand.args].join(" ")}</code>
          </p>
        )}
        <div className="row">
          <button className="secondary small" disabled={disabled} onClick={() => start.mutate({ kind: "check" })}>
            {t("tests.check")}
          </button>
          <button
            className="secondary small"
            disabled={disabled}
            onClick={() => {
              if (window.confirm(t("tests.networkConfirm"))) start.mutate({ kind: "setup", allow_network: true });
            }}
          >
            {t("tests.setup")}
          </button>
        </div>
        <form
          className="row"
          onSubmit={(e) => {
            e.preventDefault();
            if (custom.trim()) start.mutate({ kind: "custom", command: custom.trim() });
          }}
        >
          <input className="grow mono" value={custom} onChange={(e) => setCustom(e.target.value)} placeholder={t("tests.customPlaceholder")} aria-label={t("tests.custom")} />
          <button className="secondary small" disabled={disabled || !custom.trim()}>
            {t("tests.runCustom")}
          </button>
        </form>
      </div>
      <ErrorBox error={start.error ?? runs.error} />

      <h3>{t("tests.history")}</h3>
      {runs.data?.length ? (
        <ul className="runs">
          {runs.data.map((r) => (
            <Run key={r.id} run={r} />
          ))}
        </ul>
      ) : (
        <Empty>{t("tests.none")}</Empty>
      )}
    </div>
  );
}
