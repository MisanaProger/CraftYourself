import { useQuery } from "@tanstack/react-query";
import { ErrorBox } from "../../components/ui";
import { TaskTable } from "../../features/task-management/TaskTable";
import { api } from "../../services/api";

export function TasksPage() {
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const names = Object.fromEntries((projects.data ?? []).map((p) => [p.id, p.name]));
  return (
    <>
      <h1>Задачи</h1>
      <ErrorBox error={tasks.error} />
      {tasks.data && <TaskTable tasks={tasks.data} projectNames={names} />}
    </>
  );
}
