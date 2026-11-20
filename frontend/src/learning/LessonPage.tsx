import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { useTheme } from "../app/theme";
import { Markdown } from "../chat/Markdown";
import { useT } from "../i18n";
import { api, pages } from "../services/api";
import type { LessonBody, LessonView, QuizResult } from "../types/api";
import { ErrorBox, tv } from "../ui";
import { IsolatedFrame, type FrameProgress } from "../ui/IsolatedFrame";

type Of<K extends LessonBody["kind"]> = LessonView & { body: Extract<LessonBody, { kind: K }> };

function useRefreshLearning() {
  const qc = useQueryClient();
  return () => {
    qc.invalidateQueries({ queryKey: ["program"] });
    qc.invalidateQueries({ queryKey: ["programs"] });
    qc.invalidateQueries({ queryKey: ["lesson"] });
    qc.invalidateQueries({ queryKey: ["skills"] });
  };
}

function Reading({ lesson }: { lesson: Of<"reading"> }) {
  const { t } = useT();
  const refresh = useRefreshLearning();
  const done = useMutation({ mutationFn: () => api.completeLesson(lesson.program_id, lesson.id), onSuccess: refresh });
  return (
    <>
      <article className="card reading">
        <Markdown text={lesson.body.markdown} />
      </article>
      <div className="row">
        {lesson.status === "completed" ? (
          <span className="ok">{t("learning.read")}</span>
        ) : (
          <button className="primary" onClick={() => done.mutate()} disabled={done.isPending}>
            {t("learning.markRead")}
          </button>
        )}
      </div>
      <ErrorBox error={done.error} />
    </>
  );
}

function Quiz({ lesson }: { lesson: Of<"quiz"> }) {
  const { t, lang } = useT();
  const refresh = useRefreshLearning();
  const [answers, setAnswers] = useState<Record<string, number>>({});
  const [result, setResult] = useState<QuizResult | null>(null);
  const submit = useMutation({
    mutationFn: () => api.submitQuiz(lesson.program_id, lesson.id, answers, lang),
    onSuccess: (r) => {
      setResult(r);
      refresh();
    },
  });
  const byId = Object.fromEntries((result?.results ?? []).map((r) => [r.id, r]));
  return (
    <form
      className="quiz"
      onSubmit={(e) => {
        e.preventDefault();
        submit.mutate();
      }}
    >
      {lesson.body.questions.map((q, qi) => {
        const r = byId[q.id];
        return (
          <fieldset key={q.id} className={`card question ${r ? (r.correct ? "right" : "wrong") : ""}`}>
            <legend>
              <span className="q-num">{qi + 1}.</span> <Markdown text={q.text} />
            </legend>
            {q.code && <pre className="q-code">{q.code}</pre>}
            {q.options.map((o, oi) => (
              <label key={oi} className={`option ${r && oi === r.answer ? "answer" : ""}`}>
                <input
                  type="radio"
                  name={q.id}
                  checked={answers[q.id] === oi}
                  onChange={() => {
                    setAnswers((a) => ({ ...a, [q.id]: oi }));
                    setResult(null);
                  }}
                />
                <Markdown text={o} />
              </label>
            ))}
            {r && (
              <p className={r.correct ? "ok small" : "warn small"}>
                <strong>{r.correct ? t("quiz.correct") : t("quiz.incorrect")}</strong> {r.explanation}
              </p>
            )}
          </fieldset>
        );
      })}
      <div className="row">
        <button className="primary" disabled={submit.isPending || Object.keys(answers).length === 0}>
          {result ? t("quiz.retry") : t("quiz.check")}
        </button>
        {result && (
          <strong className={result.passed ? "ok" : "warn"}>
            {t("quiz.score", { correct: result.correct, total: result.total })}{" "}
            {result.passed ? t("quiz.passed") : t("quiz.notPassed", { pct: lesson.body.pass_percent })}
          </strong>
        )}
      </div>
      <ErrorBox error={submit.error} />
    </form>
  );
}

/** Standalone artifact; its progress is validated and forwarded to the server. */
function Artifact({ lesson }: { lesson: Of<"artifact"> }) {
  const { t, lang } = useT();
  const { theme } = useTheme();
  const refresh = useRefreshLearning();
  const [saved, setSaved] = useState<number | null>(null);
  const progress = useMutation({
    mutationFn: ({ score, completed }: FrameProgress) => api.artifactProgress(lesson.program_id, lesson.id, score, completed),
    onSuccess: (p) => {
      setSaved(p.score);
      refresh();
    },
  });
  const src = `/api/v1/learning/artifacts/${encodeURIComponent(lesson.body.artifact_id)}/${lesson.body.entrypoint}?lang=${lang}&theme=${theme}`;
  return (
    <>
      {lesson.body.objectives.length > 0 && (
        <ul className="objectives">
          {lesson.body.objectives.map((o) => (
            <li key={o}>{o}</li>
          ))}
        </ul>
      )}
      <IsolatedFrame src={src} title={lesson.title} initialHeight={720} onProgress={progress.mutate} />
      <p className="muted small">
        {t("learning.artifactNote")} {saved != null && <span className="ok">{t("learning.saved", { score: saved })}</span>}
      </p>
      <ErrorBox error={progress.error} />
    </>
  );
}

/** An HTML5 page of a generated program: it may report progress, or the learner marks it done. */
function Page({ lesson }: { lesson: Of<"page"> }) {
  const { t } = useT();
  const { theme } = useTheme();
  const refresh = useRefreshLearning();
  const [saved, setSaved] = useState<number | null>(null);
  const progress = useMutation({
    mutationFn: ({ score, completed }: FrameProgress) => api.artifactProgress(lesson.program_id, lesson.id, score, completed),
    onSuccess: (p) => {
      setSaved(p.score);
      refresh();
    },
  });
  const done = useMutation({ mutationFn: () => api.completeLesson(lesson.program_id, lesson.id), onSuccess: refresh });
  return (
    <>
      <IsolatedFrame src={pages.lesson(lesson.program_id, lesson.id, theme)} title={lesson.title} initialHeight={560} onProgress={progress.mutate} />
      <p className="muted small">
        {t("learning.pageNote")} {saved != null && <span className="ok">{t("learning.saved", { score: saved })}</span>}
      </p>
      <div className="row">
        {lesson.status === "completed" ? (
          <span className="ok">{t("learning.done")}</span>
        ) : (
          <button className="primary" onClick={() => done.mutate()} disabled={done.isPending}>
            {t("learning.markDone")}
          </button>
        )}
      </div>
      <ErrorBox error={progress.error ?? done.error} />
    </>
  );
}

function Practice({ lesson }: { lesson: Of<"practice"> }) {
  const { t } = useT();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const start = useMutation({
    mutationFn: () => api.startPractice(lesson.program_id, lesson.id),
    onSuccess: (s) => {
      qc.invalidateQueries({ queryKey: ["sessions"] });
      if (s.conversation_id) navigate(`/chat/${s.conversation_id}`);
    },
  });
  const b = lesson.body;
  return (
    <div className="card practice">
      <ol className="steps">
        <li>{t("practice.step1")}</li>
        <li>{t("practice.step2")}</li>
        <li>{t("practice.step3")}</li>
        <li>{t("practice.step4")}</li>
      </ol>
      {b.conversation_id ? (
        <div className="row">
          <Link className="button primary" to={`/chat/${b.conversation_id}`}>
            {t("practice.continue")}
          </Link>
          {b.task_status && <span className="muted">{tv(t, "status", b.task_status)}</span>}
        </div>
      ) : (
        <button className="primary" onClick={() => start.mutate()} disabled={start.isPending}>
          {start.isPending ? t("practice.creating") : t("practice.start")}
        </button>
      )}
      <ErrorBox error={start.error} />
    </div>
  );
}

export function LessonPage() {
  const { program = "", lesson = "" } = useParams();
  const { t, lang } = useT();
  const view = useQuery({ queryKey: ["lesson", program, lesson, lang], queryFn: () => api.lesson(program, lesson, lang) });
  const prog = useQuery({ queryKey: ["program", program, lang], queryFn: () => api.program(program, lang) });
  if (view.error)
    return (
      <div className="page">
        <ErrorBox error={view.error} />
      </div>
    );
  const l = view.data;
  if (!l) return <p className="muted pad">{t("common.loading")}</p>;
  return (
    <div className="page narrow">
      <p className="crumbs">
        <Link to="/">{t("nav.home")}</Link> / <Link to={`/learning/${program}`}>{prog.data?.title ?? t("learning.program")}</Link>
      </p>
      <header className="page-title">
        <p className="muted small">
          {tv(t, "lessonKind", l.kind)} · {t("home.minutes", { n: l.minutes })}
        </p>
        <h1>{l.title}</h1>
        {l.intro && <p className="muted">{l.intro}</p>}
      </header>
      {l.body.kind === "reading" && <Reading lesson={l as Of<"reading">} />}
      {l.body.kind === "quiz" && <Quiz key={l.id} lesson={l as Of<"quiz">} />}
      {l.body.kind === "artifact" && <Artifact lesson={l as Of<"artifact">} />}
      {l.body.kind === "practice" && <Practice lesson={l as Of<"practice">} />}
      {l.body.kind === "page" && <Page key={l.id} lesson={l as Of<"page">} />}
      <nav className="lesson-nav">
        {l.previous ? <Link to={`/learning/${program}/${l.previous}`}>← {t("learning.previous")}</Link> : <span />}
        {l.next ? (
          <Link to={`/learning/${program}/${l.next}`}>{t("learning.next")} →</Link>
        ) : (
          <Link to={`/learning/${program}`}>{t("learning.backToProgram")}</Link>
        )}
      </nav>
    </div>
  );
}
