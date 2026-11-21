import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { CommitsReview, PublishPreview, PullRequestView } from "../types/api";
import { ErrorBox, shortHash, tv } from "../ui";
import { PublishDraft, Published } from "./PublishDraft";

function Conventions({ r }: { r: CommitsReview }) {
  const { t } = useT();
  const c = r.conventions;
  if (c.learned_from < 10) return <p className="hint">{t("review.noConventions", { n: c.learned_from })}</p>;
  const rules = [
    c.conventional && t("review.conv.conventional"),
    c.capitalized === true && t("review.conv.capital"),
    c.capitalized === false && t("review.conv.lowercase"),
    c.subject_limit === 50 && t("review.conv.short"),
    c.signed_off && t("review.conv.signoff"),
  ].filter(Boolean);
  return (
    <p className="hint">
      {t("review.conventions", { n: c.learned_from })}: {rules.length ? rules.join(" · ") : t("review.conv.none")}
    </p>
  );
}

/** Commit review: fixed rules plus the repository's own conventions. */
export function CommitReviewSection({ sessionId, version }: { sessionId: string; version: string }) {
  const { t } = useT();
  const r = useQuery({ queryKey: ["commit-review", sessionId, version], queryFn: () => api.reviewCommits(sessionId) });
  if (r.error) return <ErrorBox error={r.error} />;
  if (!r.data) return null;
  const d = r.data;
  return (
    <section className="stack">
      <h3>
        {t("review.title")} {d.score != null && <span className="muted">· {d.score}/100</span>}
      </h3>
      <Conventions r={d} />
      {d.commits.length === 0 && <p className="muted small">{t("git.noCommits")}</p>}
      <ul className="plain review-list">
        {d.commits.map((c) => (
          <li key={c.hash}>
            <div>
              <code>{shortHash(c.hash)}</code> <strong>{c.score}</strong> {c.subject || "—"}
            </div>
            {c.findings.length > 0 && (
              <ul className="findings small">
                {c.findings.map((f) => (
                  <li key={f.code} className={`finding-${f.severity}`}>
                    {tv(t, "review.finding", f.code)}
                    {f.detail && <span className="muted"> · {f.detail}</span>}
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

function PullRequestForm({ sessionId, pr }: { sessionId: string; pr: PullRequestView }) {
  const { t } = useT();
  const [title, setTitle] = useState(pr.title);
  const [body, setBody] = useState(pr.body);
  const [head, setHead] = useState(pr.branch);
  const [base, setBase] = useState(pr.base ?? "main");
  const [draft, setDraft] = useState<PublishPreview | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const prepare = useMutation({ mutationFn: () => api.draftPullRequest(sessionId, { title, body, head, base }), onSuccess: setDraft });
  if (draft) return <PublishDraft draft={draft} onClose={() => setDraft(null)} onPublished={setUrl} />;
  return (
    <form
      className="stack"
      onSubmit={(e) => {
        e.preventDefault();
        prepare.mutate();
      }}
    >
      <label className="field">
        <span>{t("pr.titleField")}</span>
        <input value={title} maxLength={200} onChange={(e) => setTitle(e.target.value)} />
      </label>
      <label className="field">
        <span>{t("pr.body")}</span>
        <textarea rows={8} className="mono" value={body} onChange={(e) => setBody(e.target.value)} />
      </label>
      <div className="row">
        <label className="field grow">
          <span>{t("pr.head")}</span>
          <input value={head} onChange={(e) => setHead(e.target.value)} />
        </label>
        <label className="field">
          <span>{t("pr.base")}</span>
          <input value={base} onChange={(e) => setBase(e.target.value)} />
        </label>
      </div>
      <p className="hint">{t("pr.pushHint", { branch: pr.branch })}</p>
      <div className="row">
        <button className="secondary small" disabled={!pr.can_publish || prepare.isPending || !title.trim()}>
          {t("pr.prepare")}
        </button>
        {!pr.can_publish && <span className="hint">{t("pr.noToken")}</span>}
      </div>
      <ErrorBox error={prepare.error} />
      <Published url={url} />
    </form>
  );
}

/** Pull request preparation: a draft from observed data and readiness. */
export function PullRequestSection({ sessionId, version }: { sessionId: string; version: string }) {
  const { t } = useT();
  const [open, setOpen] = useState(false);
  const pr = useQuery({ queryKey: ["pull-request", sessionId, version], queryFn: () => api.pullRequest(sessionId) });
  if (pr.error) return <ErrorBox error={pr.error} />;
  if (!pr.data) return null;
  const d = pr.data;
  return (
    <section className="stack">
      <h3>{t("pr.title")}</h3>
      <ul className="checklist small">
        {d.readiness.map((r) => (
          <li key={r.code}>
            <span className={r.ok ? "ok" : "warn"}>
              {r.ok ? "✓" : "!"} {tv(t, "pr.ready", r.code)}
              {r.detail && <span className="muted"> · {r.detail}</span>}
            </span>
          </li>
        ))}
      </ul>
      <p className="small">{d.ready ? t("pr.isReady") : t("pr.notReady")}</p>
      {d.repository ? (
        <>
          <p className="hint">{t("pr.repository", { repo: `${d.repository.host}/${d.repository.project}` })}</p>
          {open ? (
            <PullRequestForm sessionId={sessionId} pr={d} />
          ) : (
            <button className="secondary small" onClick={() => setOpen(true)}>
              {t("pr.open")}
            </button>
          )}
        </>
      ) : (
        <p className="hint">{t("pr.noRepository")}</p>
      )}
    </section>
  );
}
