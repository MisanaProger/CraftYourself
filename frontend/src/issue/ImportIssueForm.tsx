import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { IssuePreview } from "../types/api";
import { ErrorBox, tv } from "../ui";

function hostOf(url: string) {
  try {
    return new URL(url.trim()).host;
  } catch {
    return "";
  }
}

/** The suitability check, as found by the server's fixed rules. */
function Suitability({ preview }: { preview: IssuePreview }) {
  const { t } = useT();
  const s = preview.suitability;
  return (
    <div className="stack">
      <p className={s.verdict === "unsuitable" ? "error" : s.verdict === "caution" ? "small" : "ok small"}>{tv(t, "issueImport.verdict", s.verdict)}</p>
      {s.findings.length > 0 && (
        <ul className="goals small">
          {s.findings.map((f) => (
            <li key={f.code} className={`finding ${f.severity}`}>
              <span aria-hidden>{f.severity === "good" ? "✓" : f.severity === "blocker" ? "✕" : "!"}</span> {tv(t, "issueImport.finding", f.code)}
              {f.detail && <span className="muted"> · {f.detail}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function ImportIssueForm() {
  const { t } = useT();
  const qc = useQueryClient();
  const [url, setUrl] = useState("");
  const [allow, setAllow] = useState(false);
  const host = hostOf(url);
  const preview = useMutation({ mutationFn: () => api.previewIssue(url.trim(), allow) });
  const imp = useMutation({
    mutationFn: (p: IssuePreview) => api.importIssue({ url: p.issue.url, allow_network: allow, project_id: p.project_id ?? undefined }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["projects"] });
    },
  });
  const p = preview.data;
  return (
    <div className="stack">
      <form
        className="stack"
        onSubmit={(e) => {
          e.preventDefault();
          imp.reset();
          preview.mutate();
        }}
      >
        <label className="field">
          <span>{t("issueImport.url")}</span>
          <input
            type="url"
            value={url}
            onChange={(e) => {
              setUrl(e.target.value);
              setAllow(false);
              preview.reset();
            }}
            placeholder="https://github.com/owner/repo/issues/123"
          />
          <span className="hint">{t("issueImport.urlHint")}</span>
        </label>
        <label className="check">
          <input type="checkbox" checked={allow} disabled={!host} onChange={(e) => setAllow(e.target.checked)} />
          <span>{t("remote.allow", { host: host || "…" })}</span>
        </label>
        <div className="row">
          <button className="secondary" disabled={!url.trim() || !allow || preview.isPending}>
            {preview.isPending ? t("remote.reading") : t("issueImport.check")}
          </button>
        </div>
      </form>
      <ErrorBox error={preview.error} />
      {p && (
        <div className="stack issue-preview">
          <div>
            <strong>{p.title}</strong>
            <div className="row small muted">
              <span>
                {p.issue.project}#{p.issue.number}
              </span>
              <span>{tv(t, "taskType", p.task.task_type)}</span>
              <span>{tv(t, "difficulty", p.task.difficulty)}</span>
              {p.repository.language && <span>{p.repository.language}</span>}
              {p.repository.license && <span>{p.repository.license}</span>}
            </div>
            {p.labels.length > 0 && (
              <div className="row small">
                {p.labels.map((l) => (
                  <span key={l} className="chip">
                    {l}
                  </span>
                ))}
              </div>
            )}
          </div>
          <Suitability preview={p} />
          {p.task.acceptance_criteria.length > 0 && <p className="small muted">{t("issueImport.criteria", { n: p.task.acceptance_criteria.length })}</p>}
          {p.task_id ? (
            <p className="small">{t("issueImport.already")}</p>
          ) : (
            <div className="row">
              <button className="primary" disabled={p.suitability.verdict === "unsuitable" || imp.isPending || imp.isSuccess} onClick={() => imp.mutate(p)}>
                {imp.isPending ? t("issueImport.importing") : t("issueImport.submit")}
              </button>
              <span className="hint">{p.project_id ? t("issueImport.projectKnown") : t("issueImport.willClone")}</span>
            </div>
          )}
          {imp.isSuccess && <p className="ok small">{t("import.done", { title: imp.data.title })}</p>}
          <ErrorBox error={imp.error} />
        </div>
      )}
    </div>
  );
}
