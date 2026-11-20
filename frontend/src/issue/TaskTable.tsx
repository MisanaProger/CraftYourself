import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { api } from "../services/api";
import type { Task } from "../types/api";
import { Badge, Empty, ErrorBox, shortId } from "../ui";

export const TYPE_LABELS: Record<string, string> = {
  "bug-fix": "Исправление",
  feature: "Функция",
  refactoring: "Рефакторинг",
  documentation: "Документация",
  performance: "Производительность",
  testing: "Тестирование",
  regression: "Регрессия",
};
const DIFFICULTY: Record<string, string> = { beginner: "Начальная", intermediate: "Средняя", advanced: "Высокая" };

export function TaskTable({ tasks, projectNames }: { tasks: Task[]; projectNames?: Record<string, string> }) {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const start = useMutation({
    mutationFn: api.startTask,
    onSuccess: (s) => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["sessions"] });
      if (s.conversation_id) navigate(`/chat/${s.conversation_id}`);
    },
  });
  if (tasks.length === 0) return <Empty>Задач пока нет. Создайте задачу на странице проекта или импортируйте упражнение.</Empty>;
  return (
    <>
      <ErrorBox error={start.error} />
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Issue</th>
              {projectNames && <th>Проект</th>}
              <th>Тип</th>
              <th>Сложность</th>
              <th>Статус</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {tasks.map((t) => (
              <tr key={t.id}>
                <td>
                  <span className="issue-id">EST-{shortId(t.id)}</span> {t.title}
                  {t.source.type === "exercise" && <span className="tag">упражнение</span>}
                </td>
                {projectNames && <td>{projectNames[t.project_id] ?? "—"}</td>}
                <td>{TYPE_LABELS[t.task_type]}</td>
                <td>{DIFFICULTY[t.difficulty]}</td>
                <td>
                  <Badge value={t.status} />
                </td>
                <td className="right">
                  {t.status !== "archived" && (
                    <button className={t.status === "in-progress" ? "primary small" : "secondary small"} disabled={start.isPending} onClick={() => start.mutate(t.id)}>
                      {t.status === "in-progress" ? "Продолжить" : t.status === "done" ? "Вернуться" : "Начать"}
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
