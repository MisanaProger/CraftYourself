import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "react-router";
import { Icon } from "../app/icons";
import { useT } from "../i18n";
import { api } from "../services/api";
import { ErrorBox, PageTitle, Progress, tv } from "../ui";

export function ProgramPage() {
  const { program = "" } = useParams();
  const { t, lang } = useT();
  const view = useQuery({ queryKey: ["program", program, lang], queryFn: () => api.program(program, lang) });
  if (view.error)
    return (
      <div className="page">
        <ErrorBox error={view.error} />
      </div>
    );
  const p = view.data;
  if (!p) return <p className="muted pad">{t("common.loading")}</p>;
  const done = p.lessons.filter((l) => l.status === "completed").length;
  return (
    <div className="page narrow">
      <p className="crumbs">
        <Link to="/">{t("nav.home")}</Link> / {t("learning.program")}
      </p>
      <PageTitle sub={p.summary}>{p.title}</PageTitle>
      <div className="program-progress">
        <span>{t("home.progress", { done, total: p.lessons.length })}</span>
        <Progress value={(done / p.lessons.length) * 100} label={t("home.progressAria")} />
      </div>
      <ol className="lessons">
        {p.lessons.map((l, i) => (
          <li key={l.id} className={`lesson-row ${l.status}`}>
            <span className="lesson-num" aria-hidden>
              {l.status === "completed" ? <Icon.check /> : i + 1}
            </span>
            <Link to={`/learning/${p.id}/${l.id}`} className="lesson-link">
              <strong>{l.title}</strong>
              <span className="muted small">
                {tv(t, "lessonKind", l.kind)} · {t("home.minutes", { n: l.minutes })}
                {l.score != null && ` · ${t("learning.best", { score: l.score })}`}
              </span>
            </Link>
            <span className={`chip status-${l.status}`}>{tv(t, "lessonStatus", l.status)}</span>
          </li>
        ))}
      </ol>
    </div>
  );
}
