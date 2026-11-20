import { useMutation, useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "react-router";
import { AdiSummary, SkillSummary, TaskSummary } from "../analytics/SummaryCards";
import { ThemeToggle } from "../app/theme";
import { api } from "../services/api";
import { Card, Empty, ErrorBox, formatDate } from "../ui";

export function WorkspaceHome() {
  const navigate = useNavigate();
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions });
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const conversations = useQuery({ queryKey: ["conversations"], queryFn: api.conversations });
  const free = useMutation({ mutationFn: () => api.createConversation(), onSuccess: (c) => navigate(`/chat/${c.id}`) });
  const title = (id: string) => tasks.data?.find((t) => t.id === id)?.title ?? id.slice(0, 8);
  const active = sessions.data?.filter((s) => s.status === "active") ?? [];
  const other = (conversations.data ?? []).filter((c) => !c.task_id).slice(0, 5);

  return (
    <div className="page">
      <header className="page-title row-between">
        <div>
          <h1>Рабочее пространство</h1>
          <p className="muted">Проект → Issue → чат и внешняя IDE → тесты → Git → оценка прогресса.</p>
        </div>
        <ThemeToggle />
      </header>
      <ErrorBox error={sessions.error ?? tasks.error ?? projects.error} />

      <Card title="Текущая работа" actions={<button className="secondary small" onClick={() => free.mutate()}>Диалог без Issue</button>}>
        {active.length === 0 ? (
          <Empty>
            Нет Issue в работе.{" "}
            {projects.data?.length ? <Link to="/tasks">Выберите задачу</Link> : <Link to="/projects">Подключите проект</Link>}, чтобы начать.
          </Empty>
        ) : (
          <ul className="list">
            {active.map((s) => (
              <li key={s.id}>
                {s.conversation_id ? <Link to={`/chat/${s.conversation_id}`}>{title(s.task_id)}</Link> : title(s.task_id)}
                <span className="muted mono small">{s.branch}</span>
                <span className="muted small">с {formatDate(s.started_at)}</span>
              </li>
            ))}
          </ul>
        )}
        {other.length > 0 && (
          <>
            <h3>Диалоги без Issue</h3>
            <ul className="list">
              {other.map((c) => (
                <li key={c.id}>
                  <Link to={`/chat/${c.id}`}>{c.title}</Link>
                  <span className="muted small">{formatDate(c.updated_at)}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </Card>

      <div className="three-col">
        <AdiSummary />
        <SkillSummary />
        <TaskSummary />
      </div>
    </div>
  );
}
