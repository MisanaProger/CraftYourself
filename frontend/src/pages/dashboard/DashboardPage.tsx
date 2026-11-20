import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { Badge, Card, Empty, ErrorBox, formatDate } from "../../components/ui";
import { api } from "../../services/api";

const LOOP = ["Проект", "Задача", "Рабочая среда", "Исследование", "Ментор", "Решение", "Тесты", "Git", "Прогресс"];

export function DashboardPage() {
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions });
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const taskTitle = (id: string) => tasks.data?.find((t) => t.id === id)?.title ?? id.slice(0, 8);
  const active = sessions.data?.filter((s) => s.status === "active") ?? [];
  const finished = sessions.data?.filter((s) => s.status !== "active").slice(0, 5) ?? [];

  return (
    <>
      <h1>Обзор</h1>
      <ol className="loop" aria-label="Учебный цикл">
        {LOOP.map((step) => (
          <li key={step}>{step}</li>
        ))}
      </ol>
      <ErrorBox error={sessions.error ?? tasks.error ?? projects.error} />
      <div className="stats">
        <div>
          <strong>{projects.data?.length ?? "—"}</strong> проектов
        </div>
        <div>
          <strong>{tasks.data?.length ?? "—"}</strong> задач
        </div>
        <div>
          <strong>{active.length}</strong> активных сессий
        </div>
      </div>

      <Card title="Текущая работа">
        {active.length === 0 ? (
          <Empty>
            Нет активных сессий. {projects.data?.length ? <Link to="/tasks">Выберите задачу</Link> : <Link to="/projects">Подключите проект</Link>}, чтобы
            начать.
          </Empty>
        ) : (
          <ul className="list">
            {active.map((s) => (
              <li key={s.id}>
                <Link to={`/sessions/${s.id}`}>{taskTitle(s.task_id)}</Link>
                <span className="muted mono">{s.branch}</span>
                <span className="muted">с {formatDate(s.started_at)}</span>
              </li>
            ))}
          </ul>
        )}
      </Card>

      {finished.length > 0 && (
        <Card title="Недавно завершённые">
          <ul className="list">
            {finished.map((s) => (
              <li key={s.id}>
                <Link to={`/sessions/${s.id}`}>{taskTitle(s.task_id)}</Link>
                <Badge value={s.status} />
                {s.finished_at && <span className="muted">{formatDate(s.finished_at)}</span>}
              </li>
            ))}
          </ul>
        </Card>
      )}
    </>
  );
}
