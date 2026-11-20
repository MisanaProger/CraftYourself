import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { SessionSummaryPanel } from "../analytics/SessionSummaryCard";
import { Icon } from "../app/icons";
import { Markdown } from "../chat/Markdown";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { Criterion, LearningSession, Task } from "../types/api";
import { Badge, Empty, ErrorBox, shortId } from "../ui";

function Criteria({ task, active, onAsk }: { task: Task; active: boolean; onAsk: (text: string) => void }) {
  const { t } = useT();
  const qc = useQueryClient();
  const [draft, setDraft] = useState("");
  const save = useMutation({
    mutationFn: (criteria: Criterion[]) => api.setCriteria(task.id, criteria),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["conversation"] });
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });
  const list = task.acceptance_criteria;
  const done = list.filter((c) => c.done).length;
  return (
    <section className="criteria">
      <h3>
        {t("criteria.title")} {list.length > 0 && <span className="muted">({done}/{list.length})</span>}
      </h3>
      {list.length === 0 && <p className="muted small">{t("criteria.empty")}</p>}
      <ul className="checklist">
        {list.map((c, i) => (
          <li key={i}>
            <label>
              <input
                type="checkbox"
                checked={c.done}
                disabled={save.isPending}
                onChange={() => save.mutate(list.map((x, j) => (j === i ? { ...x, done: !x.done } : x)))}
              />
              <span className={c.done ? "done" : ""}>
                <Markdown text={c.text} />
              </span>
            </label>
            <button className="icon-btn small" aria-label={t("criteria.remove")} onClick={() => save.mutate(list.filter((_, j) => j !== i))}>
              ×
            </button>
          </li>
        ))}
      </ul>
      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          if (!draft.trim()) return;
          save.mutate([...list, { text: draft.trim(), done: false }]);
          setDraft("");
        }}
      >
        <input className="grow" value={draft} onChange={(e) => setDraft(e.target.value)} placeholder={t("criteria.add")} aria-label={t("criteria.add")} />
      </form>
      {active && (
        <button className="secondary small" onClick={() => onAsk(t("criteria.askMessage"))}>
          {t("criteria.ask")}
        </button>
      )}
      <ErrorBox error={save.error} />
    </section>
  );
}

export function IssuePanel({ task, session, onAsk }: { task: Task | null; session: LearningSession | null; onAsk: (text: string) => void }) {
  const { t } = useT();
  const qc = useQueryClient();
  const [copied, setCopied] = useState(false);
  const overview = useQuery({ queryKey: ["session", session?.id], queryFn: () => api.session(session!.id), enabled: !!session });
  const open = useMutation({ mutationFn: () => api.openWorkspace(session!.id) });
  const complete = useMutation({
    mutationFn: () => api.completeTask(task!.id),
    onSuccess: () => {
      for (const k of ["sessions", "tasks", "adi", "program", "programs", "knowledge", "summary", "skills"]) qc.invalidateQueries({ queryKey: [k] });
      qc.invalidateQueries({ queryKey: ["session", session?.id] });
    },
  });
  if (!task) return <Empty>{t("issue.noTask")}</Empty>;
  const active = session?.status === "active";
  const o = overview.data;
  return (
    <div className="issue">
      <div className="issue-head">
        <span className="issue-id">EST-{shortId(task.id)}</span>
        <Badge value={session?.status ?? task.status} />
      </div>
      <h2>{task.title}</h2>
      {task.description && (
        <div className="muted description">
          <Markdown text={task.description.replace(/\n(?!\n)/g, " ")} />
        </div>
      )}
      <Criteria task={task} active={active} onAsk={onAsk} />
      {task.learning_goals.length > 0 && (
        <>
          <h3>{t("issue.goals")}</h3>
          <ul className="goals">
            {task.learning_goals.map((g) => (
              <li key={g}>{g}</li>
            ))}
          </ul>
        </>
      )}
      {task.docs.length > 0 && (
        <>
          <h3>{t("issue.sources")}</h3>
          <ul className="goals">
            {task.docs.map((d) => (
              <li key={d.url}>
                <a href={d.url} target="_blank" rel="noreferrer noopener">
                  {d.title}
                </a>
              </li>
            ))}
          </ul>
        </>
      )}
      {session && (
        <dl className="kv">
          <dt>Git</dt>
          <dd>
            <Icon.branch />
            <code title={session.branch}>{o?.current_branch ?? session.branch}</code>
            <button
              className="icon-btn small"
              aria-label={t("issue.copyPath")}
              title={copied ? t("issue.copied") : t("issue.copyPath")}
              onClick={async () => {
                await navigator.clipboard?.writeText(session.workspace_path);
                setCopied(true);
              }}
            >
              <Icon.copy />
            </button>
          </dd>
          <dt>{t("issue.changes")}</dt>
          <dd>{o ? t("issue.changesValue", { commits: o.commits.length, files: o.changes.length }) : "…"}</dd>
        </dl>
      )}
      {active && (
        <div className="stack">
          <button className="secondary" onClick={() => open.mutate()} disabled={open.isPending}>
            {t("issue.openIde")}
          </button>
          <button className="primary" onClick={() => complete.mutate()} disabled={complete.isPending}>
            {t("issue.complete")}
          </button>
          <p className="hint">{t("issue.completeHint")}</p>
        </div>
      )}
      {session && !active && <SessionSummaryPanel sessionId={session.id} />}
      <ErrorBox error={open.error ?? complete.error} />
      {open.data && <p className="ok small">{t("issue.launched", { command: open.data.command })}</p>}
    </div>
  );
}
