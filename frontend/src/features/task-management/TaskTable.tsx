import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { Badge, Empty, ErrorBox } from "../../components/ui";
import { api } from "../../services/api";
import type { Task } from "../../types/api";

const TYPE_LABELS: Record<string, string> = {
  "bug-fix": "исправление",
  feature: "функция",
  refactoring: "рефакторинг",
  documentation: "документация",
  performance: "производительность",
  testing: "тестирование",
};

export function TaskTable({ tasks, projectNames }: { tasks: Task[]; projectNames?: Record<string, string> }) {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const start = useMutation({
    mutationFn: api.startTask,
    onSuccess: (s) => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["sessions"] });
      navigate(`/sessions/${s.id}`);
    },
  });

  if (tasks.length === 0) return <Empty>Задач пока нет.</Empty>;
  return (
    <>
      <ErrorBox error={start.error} />
      <div className="table-wrap">
      <table className="tasks">
        <thead>
          <tr>
            <th>Задача</th>
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
                {t.title}
                {t.source.type === "exercise" && <span className="tag tag-quiet">упражнение</span>}
              </td>
              {projectNames && <td>{projectNames[t.project_id] ?? "—"}</td>}
              <td>{TYPE_LABELS[t.task_type]}</td>
              <td>{t.difficulty}</td>
              <td>
                <Badge value={t.status} />
              </td>
              <td className="right">
                {t.status !== "archived" && (
                  <button className="small" disabled={start.isPending} onClick={() => start.mutate(t.id)}>
                    {t.status === "in-progress" ? "Продолжить" : "Начать"}
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
