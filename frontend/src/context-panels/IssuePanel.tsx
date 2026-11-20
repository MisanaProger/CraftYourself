import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Icon } from "../app/icons";
import { api } from "../services/api";
import type { LearningSession, Task } from "../types/api";
import { Badge, Empty, ErrorBox, shortId } from "../ui";

export function IssuePanel({ task, session }: { task: Task | null; session: LearningSession | null }) {
  const qc = useQueryClient();
  const [copied, setCopied] = useState(false);
  const overview = useQuery({ queryKey: ["session", session?.id], queryFn: () => api.session(session!.id), enabled: !!session });
  const open = useMutation({ mutationFn: () => api.openWorkspace(session!.id) });
  const complete = useMutation({
    mutationFn: () => api.completeTask(task!.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["sessions"] });
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["adi"] });
      qc.invalidateQueries({ queryKey: ["session", session?.id] });
    },
  });
  if (!task) return <Empty>Диалог не связан с Issue: здесь действует отдельный уровень помощи.</Empty>;
  const active = session?.status === "active";
  const o = overview.data;
  return (
    <div className="issue">
      <div className="issue-head">
        <span className="issue-id">EST-{shortId(task.id)}</span>
        <Badge value={session?.status ?? task.status} />
      </div>
      <h2>{task.title}</h2>
      {task.description && <p className="muted">{task.description}</p>}
      {task.learning_goals.length > 0 && (
        <>
          <h3>Учебные цели</h3>
          <ul className="goals">
            {task.learning_goals.map((g) => (
              <li key={g}>{g}</li>
            ))}
          </ul>
        </>
      )}
      {task.docs.length > 0 && (
        <>
          <h3>Первоисточники</h3>
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
              aria-label="Копировать путь к рабочему каталогу"
              title={copied ? "Скопировано" : "Копировать путь к рабочему каталогу"}
              onClick={async () => {
                await navigator.clipboard?.writeText(session.workspace_path);
                setCopied(true);
              }}
            >
              <Icon.copy />
            </button>
          </dd>
          <dt>Изменения</dt>
          <dd>
            {o ? `${o.commits.length} своих коммитов · ${o.changes.length} незафиксированных файлов` : "…"}
          </dd>
        </dl>
      )}
      {active && (
        <div className="stack">
          <button className="secondary" onClick={() => open.mutate()} disabled={open.isPending}>
            Открыть в IDE
          </button>
          <button className="primary" onClick={() => complete.mutate()} disabled={complete.isPending}>
            Завершить Issue
          </button>
          <p className="hint">После завершения выставляется ADI по наблюдаемым событиям.</p>
        </div>
      )}
      <ErrorBox error={open.error ?? complete.error} />
      {open.data && <p className="ok small">Запущено: {open.data.command}</p>}
    </div>
  );
}
