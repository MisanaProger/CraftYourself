import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router";
import { Icon } from "../app/icons";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { Recommendation } from "../types/api";
import { ErrorBox, tv, useFormat } from "../ui";

function ActionButton({ r }: { r: Recommendation }) {
  const { t, lang } = useT();
  const qc = useQueryClient();
  const generate = useMutation({
    mutationFn: (a: { topic: string; level: "beginner" | "intermediate" | "advanced" }) => api.generateProgram({ topic: a.topic, level: a.level, lessons: 6, lang }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["generated"] }),
  });
  const a = r.action;
  if (!a) return null;
  switch (a.kind) {
    case "open_program":
      return (
        <Link className="button secondary small" to={`/learning/${a.program_id}`}>
          {t("rec.openProgram")}: {a.title}
        </Link>
      );
    case "continue_task":
      return (
        <Link className="button secondary small" to={a.conversation_id ? `/chat/${a.conversation_id}` : "/tasks"}>
          {t("rec.continue")}: {a.title}
        </Link>
      );
    case "solve_problems":
      return (
        <span className="row">
          <a className="button secondary small" href={a.url} target="_blank" rel="noreferrer noopener">
            {t("rec.findProblems")} ↗
          </a>
          {a.tags.map((tag) => (
            <span key={tag} className="chip">
              {tag}
            </span>
          ))}
          {a.rating != null && <span className="chip">{a.rating}</span>}
          {a.difficulty && <span className="chip">{tv(t, "problems.diff", a.difficulty)}</span>}
        </span>
      );
    case "generate_program":
      return generate.isSuccess ? (
        <span className="ok small">{t("rec.generated")}</span>
      ) : (
        <span className="stack rec-action">
          <button className="secondary small" disabled={generate.isPending} onClick={() => generate.mutate(a)}>
            {t("rec.generate")}: {a.topic}
          </button>
          <ErrorBox error={generate.error} />
        </span>
      );
  }
}

function WhatTheModelSees() {
  const { t, lang } = useT();
  const [open, setOpen] = useState(false);
  const activity = useQuery({ queryKey: ["activity", lang], queryFn: () => api.learningActivity(lang), enabled: open });
  const privacy = useQuery({ queryKey: ["privacy"], queryFn: api.privacy });
  return (
    <details className="rec-data" onToggle={(e) => setOpen((e.target as HTMLDetailsElement).open)}>
      <summary>{t("rec.data")}</summary>
      <p className="hint">
        {t("rec.dataHint")} {privacy.data?.llm?.remote && <strong className="warn">{t("rec.remote")}</strong>}
      </p>
      <ErrorBox error={activity.error} />
      {activity.data != null && <pre className="mono rec-json">{JSON.stringify(activity.data, null, 2)}</pre>}
    </details>
  );
}

/** Next steps a model suggests from the learner's activity. */
export function Recommendations() {
  const { t, lang } = useT();
  const fmt = useFormat();
  const qc = useQueryClient();
  const latest = useQuery({ queryKey: ["recommendations"], queryFn: api.recommendations });
  const ask = useMutation({
    mutationFn: () => api.recommend(lang),
    onSuccess: (set) => qc.setQueryData(["recommendations"], set),
  });
  const set = latest.data;
  return (
    <section className="card recommendations" aria-label={t("rec.title")}>
      <div className="card-head">
        <h2 className="section-title row">
          <Icon.spark /> {t("rec.title")}
        </h2>
        <button className={set ? "secondary small" : "primary"} disabled={ask.isPending} onClick={() => ask.mutate()}>
          {ask.isPending ? t("rec.working") : set ? t("rec.refresh") : t("rec.get")}
        </button>
      </div>
      {!set && <p className="muted small">{t("rec.subtitle")}</p>}
      <ErrorBox error={latest.error ?? ask.error} />
      {set && (
        <>
          {set.summary && <p>{set.summary}</p>}
          <ol className="rec-list">
            {set.items.map((r, i) => (
              <li key={i}>
                <strong>{r.title}</strong>
                {r.reason && <p className="muted small">{r.reason}</p>}
                <ActionButton r={r} />
              </li>
            ))}
          </ol>
          <p className="hint mono">{t("rec.from", { model: set.model, time: fmt.date(set.created_at) })}</p>
        </>
      )}
      <WhatTheModelSees />
    </section>
  );
}
