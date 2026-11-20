import { useQuery } from "@tanstack/react-query";
import { api } from "../services/api";
import { Card, ErrorBox, PageTitle } from "../ui";
import { TaskTable } from "./TaskTable";

export function TasksPage() {
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const names = Object.fromEntries((projects.data ?? []).map((p) => [p.id, p.name]));
  return (
    <div className="page">
      <PageTitle sub="Каждая новая Issue начинается с уровня помощи L0 и получает свой рабочий каталог.">Задачи</PageTitle>
      <ErrorBox error={tasks.error} />
      <Card>{tasks.data && <TaskTable tasks={tasks.data} projectNames={names} />}</Card>
    </div>
  );
}
