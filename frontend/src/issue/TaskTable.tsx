import { useT } from "../i18n";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { api } from "../services/api";
import type { Task } from "../types/api";
import { Badge, Empty, ErrorBox, shortId, tv } from "../ui";

export const TASK_TYPES = ["bug-fix", "feature", "refactoring", "documentation", "performance", "testing", "regression"] as const;

export function TaskTable({ tasks, projectNames }: { tasks: Task[]; projectNames?: Record<string, string> }) {
  const { t } = useT();
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
  if (tasks.length === 0) return <Empty>{t("tasks.empty")}</Empty>;
  return (
    <>
      <ErrorBox error={start.error} />
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Issue</th>
              {projectNames && <th>{t("tasks.project")}</th>}
              <th>{t("tasks.type")}</th>
              <th>{t("tasks.difficulty")}</th>
              <th>{t("tasks.status")}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {tasks.map((task) => (
              <tr key={task.id}>
                <td>
                  <span className="issue-id">EST-{shortId(task.id)}</span> {task.title}
                  {task.source.type === "exercise" && <span className="tag">{t("tasks.exercise")}</span>}
                </td>
                {projectNames && <td>{projectNames[task.project_id] ?? "—"}</td>}
                <td>{tv(t, "taskType", task.task_type)}</td>
                <td>{tv(t, "difficulty", task.difficulty)}</td>
                <td>
                  <Badge value={task.status} />
                </td>
                <td className="right">
                  {task.status !== "archived" && (
                    <button className={task.status === "in-progress" ? "primary small" : "secondary small"} disabled={start.isPending} onClick={() => start.mutate(task.id)}>
                      {task.status === "in-progress" ? t("tasks.continue") : task.status === "done" ? t("tasks.reopen") : t("tasks.start")}
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
