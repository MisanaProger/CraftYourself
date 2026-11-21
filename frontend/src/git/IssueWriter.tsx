import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { IssueTextKind, PublishPreview } from "../types/api";
import { ErrorBox, tv } from "../ui";
import { PublishDraft, Published } from "./PublishDraft";

/**
 * Writing an Issue for the project's repository: a checklist computed on
 * the server, then a draft confirmed before anything is published.
 */
export function IssueWriter({ projectId, hasRepository }: { projectId: string; hasRepository: boolean }) {
  const { t } = useT();
  const [kind, setKind] = useState<IssueTextKind>("bug");
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [draft, setDraft] = useState<PublishPreview | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const check = useMutation({ mutationFn: () => api.reviewIssueText(kind, title, body) });
  const prepare = useMutation({ mutationFn: () => api.draftNewIssue(projectId, kind, title, body), onSuccess: setDraft });
  const r = check.data;
  if (draft) return <PublishDraft draft={draft} onClose={() => setDraft(null)} onPublished={setUrl} />;
  return (
    <form
      className="stack"
      onSubmit={(e) => {
        e.preventDefault();
        check.mutate();
      }}
    >
      <div className="row">
        <label className="field">
          <span>{t("issueText.kind")}</span>
          <select value={kind} onChange={(e) => setKind(e.target.value as IssueTextKind)}>
            <option value="bug">{t("issueText.bug")}</option>
            <option value="feature">{t("issueText.feature")}</option>
          </select>
        </label>
        <label className="field grow">
          <span>{t("issueText.titleField")}</span>
          <input value={title} maxLength={200} onChange={(e) => setTitle(e.target.value)} />
        </label>
      </div>
      <label className="field">
        <span>{t("issueText.body")}</span>
        <textarea rows={8} value={body} onChange={(e) => setBody(e.target.value)} placeholder={kind === "bug" ? t("issueText.bugPlaceholder") : t("issueText.featurePlaceholder")} />
      </label>
      <div className="row">
        <button className="secondary small" disabled={check.isPending}>
          {t("issueText.check")}
        </button>
        {hasRepository && (
          <button type="button" className="secondary small" disabled={prepare.isPending || !title.trim()} onClick={() => prepare.mutate()}>
            {t("issueText.prepare")}
          </button>
        )}
      </div>
      {!hasRepository && <p className="hint">{t("issueText.noRepository")}</p>}
      {r && (
        <div className="stack">
          <p className="small">{t("issueText.score", { n: r.score })}</p>
          <ul className="checklist small">
            {r.blockers.map((b) => (
              <li key={b.code} className="error">
                ✕ {tv(t, "issueText.blocker", b.code)}
              </li>
            ))}
            {r.checks.map((c) => (
              <li key={c.code}>
                <span className={c.ok ? "ok" : "warn"}>
                  {c.ok ? "✓" : "!"} {tv(t, "issueText.check", c.code)}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
      <ErrorBox error={check.error ?? prepare.error} />
      <Published url={url} />
    </form>
  );
}
