import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate } from "react-router";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { ImportProblem, ProblemLanguage, ProblemSummary, Sample } from "../types/api";
import { Badge, Card, Empty, ErrorBox, PageTitle, tv } from "../ui";

const LANGUAGES: ProblemLanguage[] = ["python", "cpp", "rust"];
const PLATFORM: Record<ProblemSummary["platform"], string> = { codeforces: "Codeforces", leetcode: "LeetCode", other: "" };

function ImportForm() {
  const { t, lang } = useT();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [url, setUrl] = useState("");
  const [title, setTitle] = useState("");
  const [statement, setStatement] = useState("");
  const [samples, setSamples] = useState<Sample[]>([{ input: "", output: "" }]);
  const [language, setLanguage] = useState<ProblemLanguage>("python");
  const [tags, setTags] = useState("");
  const [rating, setRating] = useState("");
  const [difficulty, setDifficulty] = useState<"" | "easy" | "medium" | "hard">("");
  const [allow, setAllow] = useState(false);
  let host = "";
  try {
    host = url.trim() ? new URL(url.trim()).host : "";
  } catch {
    host = "";
  }
  const fetchable = /^(www\.)?(codeforces\.com|leetcode\.com)$/.test(host);
  // Fills the form with what the judge returned; the learner reviews it.
  const load = useMutation({
    mutationFn: () => api.fetchProblem(url.trim(), allow),
    onSuccess: ({ input }) => {
      if (input.title) setTitle(input.title);
      if (input.statement) setStatement(input.statement);
      if (input.samples.length) setSamples(input.samples);
      if (input.tags.length) setTags(input.tags.join(", "));
      if (input.rating) setRating(String(input.rating));
      if (input.difficulty) setDifficulty(input.difficulty);
    },
  });
  const setSample = (i: number, s: Partial<Sample>) => setSamples((all) => all.map((x, k) => (k === i ? { ...x, ...s } : x)));
  const create = useMutation({
    mutationFn: () => {
      const req: ImportProblem = {
        url: url.trim() || undefined,
        title: title.trim() || undefined,
        statement,
        samples: samples.filter((s) => s.input.trim() || s.output.trim()),
        tags: tags.split(",").map((x) => x.trim()).filter(Boolean),
        rating: rating ? Number(rating) : undefined,
        difficulty: difficulty || undefined,
        language,
        lang,
      };
      return api.importProblem(req);
    },
    onSuccess: (s) => {
      qc.invalidateQueries({ queryKey: ["problems"] });
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["sessions"] });
      if (s.conversation_id) navigate(`/chat/${s.conversation_id}`);
    },
  });
  return (
    <form
      className="stack problem-form"
      onSubmit={(e) => {
        e.preventDefault();
        create.mutate();
      }}
    >
      <label className="field">
        <span>{t("problems.url")}</span>
        <input
          type="url"
          value={url}
          onChange={(e) => {
            setUrl(e.target.value);
            setAllow(false);
            load.reset();
          }}
          placeholder="https://codeforces.com/problemset/problem/4/A"
        />
        <span className="hint">{t("problems.urlHint")}</span>
      </label>
      {fetchable && (
        <div className="stack">
          <label className="check">
            <input type="checkbox" checked={allow} onChange={(e) => setAllow(e.target.checked)} />
            <span>{t("remote.allow", { host })}</span>
          </label>
          <div className="row">
            <button type="button" className="secondary small" disabled={!allow || load.isPending} onClick={() => load.mutate()}>
              {load.isPending ? t("remote.reading") : t("problems.load")}
            </button>
            {load.data && <span className={load.data.statement_found ? "ok small" : "small"}>{load.data.statement_found ? t("problems.loaded") : t("problems.loadedPartly")}</span>}
          </div>
          <ErrorBox error={load.error} />
        </div>
      )}
      <label className="field">
        <span>{t("problems.titleField")}</span>
        <input value={title} maxLength={120} onChange={(e) => setTitle(e.target.value)} />
      </label>
      <label className="field">
        <span>{t("problems.statement")}</span>
        <textarea rows={10} value={statement} onChange={(e) => setStatement(e.target.value)} required />
        <span className="hint">{t("problems.statementHint")}</span>
      </label>
      <fieldset className="samples">
        <legend>{t("problems.samples")}</legend>
        <p className="hint">{t("problems.samplesHint")}</p>
        {samples.map((s, i) => (
          <div key={i} className="sample-row">
            <span className="sample-num" aria-hidden>
              {i + 1}
            </span>
            <label className="field">
              <span>{t("problems.input")}</span>
              <textarea className="mono" rows={3} value={s.input} onChange={(e) => setSample(i, { input: e.target.value })} />
            </label>
            <label className="field">
              <span>{t("problems.output")}</span>
              <textarea className="mono" rows={3} value={s.output} onChange={(e) => setSample(i, { output: e.target.value })} />
            </label>
            <button
              type="button"
              className="icon-btn small"
              aria-label={t("problems.removeSample", { n: i + 1 })}
              title={t("problems.removeSample", { n: i + 1 })}
              onClick={() => setSamples((all) => all.filter((_, k) => k !== i))}
            >
              ✕
            </button>
          </div>
        ))}
        {samples.length < 10 && (
          <button type="button" className="secondary small" onClick={() => setSamples((all) => [...all, { input: "", output: "" }])}>
            {t("problems.addSample")}
          </button>
        )}
      </fieldset>
      <div className="row problem-options">
        <label className="field">
          <span>{t("problems.language")}</span>
          <select value={language} onChange={(e) => setLanguage(e.target.value as ProblemLanguage)}>
            {LANGUAGES.map((l) => (
              <option key={l} value={l}>
                {tv(t, "problems.lang", l)}
              </option>
            ))}
          </select>
        </label>
        <label className="field grow">
          <span>{t("problems.tags")}</span>
          <input value={tags} onChange={(e) => setTags(e.target.value)} placeholder="math, brute force" />
        </label>
        <label className="field">
          <span>{t("problems.rating")} · Codeforces</span>
          <input className="narrow-input" type="number" min={800} max={3500} step={100} value={rating} onChange={(e) => setRating(e.target.value)} />
        </label>
        <label className="field">
          <span>{t("problems.difficulty")} · LeetCode</span>
          <select value={difficulty} onChange={(e) => setDifficulty(e.target.value as typeof difficulty)}>
            <option value="">—</option>
            {(["easy", "medium", "hard"] as const).map((d) => (
              <option key={d} value={d}>
                {tv(t, "problems.diff", d)}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="row">
        <button className="primary" disabled={create.isPending || !statement.trim()}>
          {create.isPending ? t("problems.creating") : t("problems.submit")}
        </button>
      </div>
      <ErrorBox error={create.error} />
    </form>
  );
}

function ProblemList() {
  const { t } = useT();
  const list = useQuery({ queryKey: ["problems"], queryFn: api.problems });
  if (list.error) return <ErrorBox error={list.error} />;
  if (!list.data) return <p className="muted">{t("common.loading")}</p>;
  if (list.data.length === 0) return <Empty>{t("problems.none")}</Empty>;
  return (
    <ul className="problem-list">
      {list.data.map((p) => (
        <li key={p.task_id} className="problem-item">
          <div>
            <strong>{p.title}</strong>
            <div className="row small muted">
              {PLATFORM[p.platform] && <span className="chip">{PLATFORM[p.platform]}</span>}
              <span>{tv(t, "difficulty", p.difficulty)}</span>
              <Badge value={p.status} />
            </div>
          </div>
          <div className="row">
            {p.url && (
              <a href={p.url} target="_blank" rel="noreferrer noopener" className="small">
                {t("problems.onJudge")} ↗
              </a>
            )}
            {p.conversation_id && (
              <Link className="button secondary small" to={`/chat/${p.conversation_id}`}>
                {t("problems.open")}
              </Link>
            )}
          </div>
        </li>
      ))}
    </ul>
  );
}

export function ProblemsPage() {
  const { t } = useT();
  return (
    <div className="page">
      <PageTitle sub={t("problems.subtitle")}>{t("problems.title")}</PageTitle>
      <div className="problems-layout">
        <Card title={t("problems.import")}>
          <ImportForm />
        </Card>
        <Card title={t("problems.list")}>
          <ProblemList />
        </Card>
      </div>
    </div>
  );
}
