import { useT } from "../i18n";
import { useQuery } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { DiscoveryPanel } from "../discovery/DiscoveryPanel";
import { api } from "../services/api";
import { Card, ErrorBox, PageTitle } from "../ui";
import { ImportIssueForm } from "./ImportIssueForm";
import { TaskTable } from "./TaskTable";

export function TasksPage() {
  const { t } = useT();
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const names = Object.fromEntries((projects.data ?? []).map((p) => [p.id, p.name]));
  // A suggested Issue goes to the import form, which checks it in full.
  const [params] = useSearchParams();
  const [picked, setPicked] = useState(params.get("issue") ?? "");
  const importRef = useRef<HTMLDivElement>(null);
  const pick = (url: string) => {
    setPicked(url);
    importRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  };
  return (
    <div className="page">
      <PageTitle sub={t("tasks.subtitle")}>{t("nav.tasks")}</PageTitle>
      <ErrorBox error={tasks.error} />
      <Card>{tasks.data && <TaskTable tasks={tasks.data} projectNames={names} />}</Card>
      <Card title={t("discovery.title")}>
        <DiscoveryPanel onPickIssue={pick} />
      </Card>
      <div ref={importRef}>
        <Card title={t("issueImport.title")}>
          <ImportIssueForm key={picked} initialUrl={picked} />
        </Card>
      </div>
    </div>
  );
}
