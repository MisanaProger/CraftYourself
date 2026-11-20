import { useMutation, useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "react-router";
import { useT } from "../i18n";
import { api } from "../services/api";
import { Card, Empty, ErrorBox, PageTitle, useFormat } from "../ui";

export function WorkspacePage() {
  const { t } = useT();
  const fmt = useFormat();
  const navigate = useNavigate();
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions });
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const conversations = useQuery({ queryKey: ["conversations"], queryFn: api.conversations });
  const free = useMutation({ mutationFn: () => api.createConversation(t("workspace.newConversation")), onSuccess: (c) => navigate(`/chat/${c.id}`) });
  const title = (id: string) => tasks.data?.find((x) => x.id === id)?.title ?? id.slice(0, 8);
  const active = sessions.data?.filter((s) => s.status === "active") ?? [];
  const other = (conversations.data ?? []).filter((c) => !c.task_id).slice(0, 8);

  return (
    <div className="page">
      <PageTitle sub={t("workspace.subtitle")}>{t("nav.workspace")}</PageTitle>
      <ErrorBox error={sessions.error ?? tasks.error ?? projects.error ?? free.error} />
      <Card
        title={t("workspace.current")}
        actions={
          <button className="secondary small" onClick={() => free.mutate()}>
            {t("workspace.freeChat")}
          </button>
        }
      >
        {active.length === 0 ? (
          <Empty>
            {t("workspace.none")}{" "}
            {projects.data?.length ? <Link to="/tasks">{t("workspace.pickTask")}</Link> : <Link to="/projects">{t("workspace.connectProject")}</Link>}
          </Empty>
        ) : (
          <ul className="list">
            {active.map((s) => (
              <li key={s.id}>
                {s.conversation_id ? <Link to={`/chat/${s.conversation_id}`}>{title(s.task_id)}</Link> : title(s.task_id)}
                <span className="muted mono small">{s.branch}</span>
                <span className="muted small">{fmt.date(s.started_at)}</span>
              </li>
            ))}
          </ul>
        )}
        {other.length > 0 && (
          <>
            <h3>{t("workspace.freeChats")}</h3>
            <ul className="list">
              {other.map((c) => (
                <li key={c.id}>
                  <Link to={`/chat/${c.id}`}>{c.title}</Link>
                  <span className="muted small">{fmt.date(c.updated_at)}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </Card>
    </div>
  );
}
