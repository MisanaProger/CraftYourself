import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router";
import { Icon } from "../app/icons";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { GeneratedProgram, GenerationLevel } from "../types/api";
import { ErrorBox, Progress, tv } from "../ui";

const LEVELS: GenerationLevel[] = ["beginner", "intermediate", "advanced"];

function GenerateForm() {
  const { t, lang } = useT();
  const qc = useQueryClient();
  const [topic, setTopic] = useState("");
  const [level, setLevel] = useState<GenerationLevel>("beginner");
  const [lessons, setLessons] = useState(6);
  const generate = useMutation({
    mutationFn: () => api.generateProgram({ topic: topic.trim(), level, lessons, lang }),
    onSuccess: () => {
      setTopic("");
      qc.invalidateQueries({ queryKey: ["generated"] });
    },
  });
  return (
    <form
      className="card generate"
      onSubmit={(e) => {
        e.preventDefault();
        generate.mutate();
      }}
    >
      <h3>{t("gen.title")}</h3>
      <p className="muted small">{t("gen.subtitle")}</p>
      <label className="stack">
        <span className="small">{t("gen.topic")}</span>
        <input value={topic} maxLength={300} onChange={(e) => setTopic(e.target.value)} placeholder={t("gen.topicPlaceholder")} required />
      </label>
      <div className="row">
        <label className="row small">
          {t("gen.level")}
          <select value={level} onChange={(e) => setLevel(e.target.value as GenerationLevel)}>
            {LEVELS.map((l) => (
              <option key={l} value={l}>
                {tv(t, "difficulty", l)}
              </option>
            ))}
          </select>
        </label>
        <label className="row small">
          {t("gen.lessons")}
          <input className="narrow-input" type="number" min={3} max={10} value={lessons} onChange={(e) => setLessons(Math.max(3, Math.min(10, Number(e.target.value) || 6)))} />
        </label>
        <button className="primary" disabled={generate.isPending || !topic.trim()}>
          {t("gen.submit")}
        </button>
      </div>
      <ErrorBox error={generate.error} />
    </form>
  );
}

function GeneratedTile({ g }: { g: GeneratedProgram }) {
  const { t } = useT();
  const qc = useQueryClient();
  const remove = useMutation({
    mutationFn: () => api.deleteGeneratedProgram(g.id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["generated"] }),
  });
  const del = g.status !== "generating" && (
    <button
      type="button"
      className="icon-btn small"
      aria-label={t("common.delete")}
      title={t("common.delete")}
      onClick={(e) => {
        e.preventDefault();
        if (window.confirm(t("gen.deleteConfirm"))) remove.mutate();
      }}
    >
      ✕
    </button>
  );
  const top = (
    <div className="tile-top">
      <span className="tile-icon">
        <Icon.book />
      </span>
      <span className="row">
        <span className="chip">{t("gen.badge")}</span>
        {del}
      </span>
    </div>
  );
  if (g.status === "ready") {
    return (
      <Link to={`/learning/${g.id}`} className="card card-link tile">
        {top}
        <h3>{g.title ?? g.topic}</h3>
        <p className="muted small">{g.summary}</p>
        <div className="tile-meta">
          <span>{tv(t, "difficulty", g.level)}</span>
          <span>· {t("home.minutes", { n: g.minutes })}</span>
          <span>· {t("home.lessons", { n: g.lessons_total })}</span>
        </div>
        <Progress value={g.lessons_total ? (g.completed / g.lessons_total) * 100 : 0} label={t("home.progressAria")} />
        {g.model && <p className="muted small mono">{t("gen.by", { model: g.model })}</p>}
      </Link>
    );
  }
  return (
    <div className={`card tile ${g.status === "failed" ? "failed" : "disabled"}`}>
      {top}
      <h3>{g.title ?? g.topic}</h3>
      {g.status === "generating" ? (
        <>
          <p className="muted small">{g.lessons_total ? t("gen.writing", { done: g.lessons_done + 1, total: g.lessons_total }) : t("gen.planning")}</p>
          <Progress value={g.lessons_total ? (g.lessons_done / g.lessons_total) * 100 : 3} label={t("home.progressAria")} />
        </>
      ) : (
        <p className="warn small">{t("gen.failed", { error: g.error ?? "" })}</p>
      )}
      <ErrorBox error={remove.error} />
    </div>
  );
}

/** Programs the learner asked a model to write, and the form to ask for one. */
export function GeneratedPrograms() {
  const { t } = useT();
  const list = useQuery({
    queryKey: ["generated"],
    queryFn: api.generatedPrograms,
    // Events refresh it too; polling covers a dropped WebSocket.
    refetchInterval: (q) => (q.state.data?.some((g) => g.status === "generating") ? 3000 : false),
  });
  return (
    <section aria-label={t("gen.yours")}>
      <h2 className="section-title">{t("gen.yours")}</h2>
      <ErrorBox error={list.error} />
      <div className="tiles">
        <GenerateForm />
        {list.data?.map((g) => (
          <GeneratedTile key={g.id} g={g} />
        ))}
      </div>
    </section>
  );
}
