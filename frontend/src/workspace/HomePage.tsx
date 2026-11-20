import type { JSX } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { AdiSummary, SkillSummary, TaskSummary } from "../analytics/SummaryCards";
import { Icon } from "../app/icons";
import { ThemeToggle } from "../app/theme";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { ProgramTile } from "../types/api";
import { ErrorBox, Progress, tv } from "../ui";

const ICONS: Record<string, () => JSX.Element> = {
  test: Icon.test,
  git: Icon.git,
  bug: Icon.bug,
  map: Icon.map,
  book: Icon.book,
  tree: Icon.tree,
  issue: Icon.issue,
};

function Tile({ p }: { p: ProgramTile }) {
  const { t } = useT();
  const I = ICONS[p.icon] ?? Icon.book;
  const soon = p.status === "coming_soon";
  const pct = p.lessons ? (p.completed / p.lessons) * 100 : 0;
  const body = (
    <>
      <div className="tile-top">
        <span className="tile-icon">
          <I />
        </span>
        {soon ? <span className="chip soon">{t("home.soon")}</span> : p.completed > 0 && <span className="chip">{t("home.progress", { done: p.completed, total: p.lessons })}</span>}
      </div>
      <h3>{p.title}</h3>
      <p className="muted small">{p.summary}</p>
      <div className="tile-meta">
        <span>{tv(t, "difficulty", p.level)}</span>
        <span>· {t("home.minutes", { n: p.minutes })}</span>
        {!soon && <span>· {t("home.lessons", { n: p.lessons })}</span>}
      </div>
      {!soon && <Progress value={pct} label={t("home.progressAria")} />}
    </>
  );
  return soon ? (
    <div className="card tile disabled" aria-disabled>
      {body}
    </div>
  ) : (
    <Link to={`/learning/${p.id}`} className="card card-link tile">
      {body}
    </Link>
  );
}

export function HomePage() {
  const { t, lang } = useT();
  const programs = useQuery({ queryKey: ["programs", lang], queryFn: () => api.programs(lang) });
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions });
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const active = (sessions.data ?? []).filter((s) => s.status === "active" && s.conversation_id);
  const title = (id: string) => tasks.data?.find((x) => x.id === id)?.title ?? id.slice(0, 8);

  return (
    <div className="page">
      <header className="hero row-between">
        <div>
          <h1>{t("home.title")}</h1>
          <p className="muted">{t("home.subtitle")}</p>
        </div>
        <ThemeToggle />
      </header>

      {active.length > 0 && (
        <section className="continue" aria-label={t("home.continue")}>
          <h2 className="section-title">{t("home.continue")}</h2>
          <div className="continue-row">
            {active.slice(0, 3).map((s) => (
              <Link key={s.id} to={`/chat/${s.conversation_id}`} className="card card-link continue-card">
                <Icon.chat />
                <div>
                  <strong>{title(s.task_id)}</strong>
                  <span className="muted small mono">{s.branch}</span>
                </div>
              </Link>
            ))}
          </div>
        </section>
      )}

      <h2 className="section-title">{t("home.programs")}</h2>
      <ErrorBox error={programs.error} />
      <div className="tiles">
        {programs.data?.map((p) => (
          <Tile key={p.id} p={p} />
        ))}
      </div>

      <h2 className="section-title">{t("home.progressTitle")}</h2>
      <div className="three-col">
        <AdiSummary />
        <SkillSummary />
        <TaskSummary />
      </div>
    </div>
  );
}
