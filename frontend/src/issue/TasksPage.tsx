import { useT } from "../i18n";
import { useQuery } from "@tanstack/react-query";
import { api } from "../services/api";
import { Card, ErrorBox, PageTitle } from "../ui";
import { ImportIssueForm } from "./ImportIssueForm";
import { TaskTable } from "./TaskTable";

export function TasksPage() {
  const { t } = useT();
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const names = Object.fromEntries((projects.data ?? []).map((p) => [p.id, p.name]));
  return (
    <div className="page">
      <PageTitle sub={t("tasks.subtitle")}>{t("nav.tasks")}</PageTitle>
      <ErrorBox error={tasks.error} />
      <Card>{tasks.data && <TaskTable tasks={tasks.data} projectNames={names} />}</Card>
      <Card title={t("issueImport.title")}>
        <ImportIssueForm />
      </Card>
    </div>
  );
}
