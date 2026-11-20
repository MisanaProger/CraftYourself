import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useParams } from "react-router";
import { Badge, Card, Empty, ErrorBox, shortHash } from "../../components/ui";
import { MentorPanel } from "../../features/ai-mentor/MentorPanel";
import { DiffView } from "../../features/git-review/DiffView";
import { api } from "../../services/api";

export function SessionPage() {
  const { id = "" } = useParams();
  const qc = useQueryClient();
  const overview = useQuery({ queryKey: ["session", id], queryFn: () => api.session(id), refetchInterval: 5_000 });
  const [copied, setCopied] = useState(false);
  const open = useMutation({ mutationFn: () => api.openWorkspace(id) });
  const finish = useMutation({
    mutationFn: (outcome: "completed" | "abandoned") => api.finish(id, outcome),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["session", id] });
      qc.invalidateQueries({ queryKey: ["sessions"] });
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });

  if (overview.error) return <ErrorBox error={overview.error} />;
  const o = overview.data;
  if (!o) return <p className="muted">Загрузка…</p>;
  const { session, task, project } = o;
  const active = session.status === "active";
  // Changes in commits or the working tree refresh the diff.
  const version = `${o.commits[0]?.hash ?? ""}:${o.changes.map((c) => c.code + c.path).join(",")}`;

  return (
    <>
      <p className="crumbs">
        <Link to={`/projects/${project.id}`}>{project.name}</Link> / задача
      </p>
      <div className="title-row">
        <h1>{task.title}</h1>
        <Badge value={session.status} />
      </div>

      <Card
        title="Рабочий каталог"
        actions={
          <div className="row">
            <button
              className="secondary"
              onClick={async () => {
                await navigator.clipboard.writeText(session.workspace_path);
                setCopied(true);
              }}
            >
              {copied ? "Скопировано" : "Копировать путь"}
            </button>
            <button onClick={() => open.mutate()} disabled={open.isPending}>
              Открыть в IDE
            </button>
          </div>
        }
      >
        <p className="mono">{session.workspace_path}</p>
        <p className="muted small">
          Ветка <code>{o.current_branch ?? session.branch}</code> от <code>{shortHash(session.base_revision)}</code>. Работайте в своём редакторе и делайте коммиты как обычно —
          EST видит изменения через Git.
        </p>
        <ErrorBox error={open.error} />
        {open.data && <p className="ok small">Запущено: {open.data.command}</p>}
      </Card>

      <div className="two-col">
        <Card title="Задача">
          {task.description ? <p className="pre">{task.description}</p> : <Empty>Описание не задано.</Empty>}
          {task.learning_goals.length > 0 && (
            <>
              <h3>Учебные цели</h3>
              <ul>
                {task.learning_goals.map((g) => (
                  <li key={g}>{g}</li>
                ))}
              </ul>
            </>
          )}
          {task.test_command && (
            <p className="small">
              Тесты: <code>{task.test_command}</code>
            </p>
          )}
          {task.docs.length > 0 && (
            <>
              <h3>Первоисточники</h3>
              <ul>
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
        </Card>
        <Card title="Git">
          <h3>Свои коммиты ({o.commits.length})</h3>
          {o.commits.length === 0 ? (
            <Empty>Коммитов с начала задачи ещё нет.</Empty>
          ) : (
            <ul className="list compact">
              {o.commits.map((c) => (
                <li key={c.hash}>
                  <code>{shortHash(c.hash)}</code> {c.subject}
                </li>
              ))}
            </ul>
          )}
          <h3>Незафиксированные изменения ({o.changes.length})</h3>
          {o.changes.length === 0 ? (
            <Empty>Рабочий каталог чист.</Empty>
          ) : (
            <ul className="list compact">
              {o.changes.map((c) => (
                <li key={c.path}>
                  <code className="status">{c.code}</code> <span className="mono">{c.path}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>

      <Card title="Ментор">
        <MentorPanel sessionId={id} maxLevel={task.mentor.max_help_level} active={active} />
      </Card>

      <Card title="Изменения">
        <DiffView sessionId={id} version={version} />
      </Card>

      {active && (
        <Card title="Завершение">
          <div className="row">
            <button onClick={() => finish.mutate("completed")} disabled={finish.isPending}>
              Задача выполнена
            </button>
            <button className="secondary" onClick={() => finish.mutate("abandoned")} disabled={finish.isPending}>
              Прервать
            </button>
          </div>
          <ErrorBox error={finish.error} />
        </Card>
      )}
    </>
  );
}
