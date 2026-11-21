import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router";
import { useT } from "../i18n";
import { WhatTheModelSees } from "../learning/Recommendations";
import { api } from "../services/api";
import type { Discovery, IssueCandidate, ProblemCandidate } from "../types/api";
import { ErrorBox, tv, useFormat } from "../ui";

const JUDGE: Record<ProblemCandidate["platform"], string> = { codeforces: "Codeforces", leetcode: "LeetCode", other: "" };

function IssueItem({ c, onPick }: { c: IssueCandidate; onPick: (url: string) => void }) {
  const { t } = useT();
  return (
    <li className="problem-item discovery-item">
      <div className="stack">
        <a href={c.issue.url} target="_blank" rel="noreferrer noopener">
          <strong>{c.title}</strong>
        </a>
        <div className="row small muted">
          <span>
            {c.issue.project}#{c.issue.number}
          </span>
          <span>{tv(t, "taskType", c.task_type)}</span>
          <span>{tv(t, "difficulty", c.difficulty)}</span>
          {c.labels.slice(0, 4).map((l) => (
            <span key={l} className="chip">
              {l}
            </span>
          ))}
        </div>
        {c.reason && <p className="small">{c.reason}</p>}
        {c.suitability.findings.some((f) => f.severity === "caution") && (
          <p className="small muted">
            ! {c.suitability.findings.filter((f) => f.severity === "caution").map((f) => tv(t, "issueImport.finding", f.code)).join(" · ")}
          </p>
        )}
      </div>
      <button className="secondary small" onClick={() => onPick(c.issue.url)}>
        {t("discovery.checkIssue")}
      </button>
    </li>
  );
}

function ProblemItem({ c }: { c: ProblemCandidate }) {
  const { t } = useT();
  return (
    <li className="problem-item discovery-item">
      <div className="stack">
        <a href={c.url} target="_blank" rel="noreferrer noopener">
          <strong>{c.title}</strong>
        </a>
        <div className="row small muted">
          <span className="chip">{JUDGE[c.platform]}</span>
          {c.rating != null && <span>{c.rating}</span>}
          {c.difficulty && <span>{tv(t, "problems.diff", c.difficulty)}</span>}
          {c.tags.slice(0, 4).map((tag) => (
            <span key={tag} className="chip">
              {tag}
            </span>
          ))}
        </div>
        {c.reason && <p className="small">{c.reason}</p>}
      </div>
      <Link className="button secondary small" to={`/problems?url=${encodeURIComponent(c.url)}`}>
        {t("discovery.solve")}
      </Link>
    </li>
  );
}

function Results({ d, onPickIssue }: { d: Discovery; onPickIssue: (url: string) => void }) {
  const { t } = useT();
  const { date } = useFormat();
  const failed = d.searches.filter((s) => s.error);
  return (
    <div className="stack">
      <p className="small muted">
        {date(d.created_at)} · {d.model}
        {d.goal && <> · «{d.goal}»</>}
      </p>
      {d.summary && <p>{d.summary}</p>}
      {d.issues.length > 0 && (
        <>
          <h3>{t("discovery.issues")}</h3>
          <ul className="problem-list">
            {d.issues.map((c) => (
              <IssueItem key={c.issue.url} c={c} onPick={onPickIssue} />
            ))}
          </ul>
        </>
      )}
      {d.problems.length > 0 && (
        <>
          <h3>{t("discovery.problems")}</h3>
          <ul className="problem-list">
            {d.problems.map((c) => (
              <ProblemItem key={`${c.platform}-${c.id}`} c={c} />
            ))}
          </ul>
        </>
      )}
      {d.issues.length === 0 && d.problems.length === 0 && <p className="muted">{t("discovery.nothing")}</p>}
      {failed.length > 0 && (
        <p className="small muted">
          {t("discovery.failed")}: {failed.map((s) => `${s.source} — ${s.error}`).join("; ")}
        </p>
      )}
    </div>
  );
}

/**
 * Discovery: the learner states a goal or leaves it empty; the model plans
 * searches, EST runs and checks them. Candidates go to the usual import.
 */
export function DiscoveryPanel({ issues: withIssues = true, problems: withProblems = true, onPickIssue }: { issues?: boolean; problems?: boolean; onPickIssue: (url: string) => void }) {
  const { t, lang } = useT();
  const qc = useQueryClient();
  const [goal, setGoal] = useState("");
  const [issues, setIssues] = useState(withIssues);
  const [problems, setProblems] = useState(withProblems);
  const [allow, setAllow] = useState(false);
  const latest = useQuery({ queryKey: ["discovery"], queryFn: api.discovery });
  const run = useMutation({
    mutationFn: () => api.discover({ goal: goal.trim() || undefined, issues, problems, allow_network: allow, lang }),
    onSuccess: (d) => qc.setQueryData(["discovery"], d),
  });
  const hosts = [issues && "github.com", problems && "codeforces.com", problems && "leetcode.com"].filter(Boolean).join(", ");
  return (
    <div className="stack">
      <form
        className="stack"
        onSubmit={(e) => {
          e.preventDefault();
          run.mutate();
        }}
      >
        <label className="field">
          <span>{t("discovery.goal")}</span>
          <textarea rows={2} maxLength={600} value={goal} onChange={(e) => setGoal(e.target.value)} placeholder={t("discovery.goalPlaceholder")} />
          <span className="hint">{t("discovery.goalHint")}</span>
        </label>
        <div className="row">
          <label className="check">
            <input type="checkbox" checked={issues} onChange={(e) => setIssues(e.target.checked)} />
            <span>{t("discovery.wantIssues")}</span>
          </label>
          <label className="check">
            <input type="checkbox" checked={problems} onChange={(e) => setProblems(e.target.checked)} />
            <span>{t("discovery.wantProblems")}</span>
          </label>
        </div>
        <label className="check">
          <input type="checkbox" checked={allow} disabled={!issues && !problems} onChange={(e) => setAllow(e.target.checked)} />
          <span>{t("discovery.allow", { hosts: hosts || "…" })}</span>
        </label>
        <div className="row">
          <button className="primary" disabled={!allow || (!issues && !problems) || run.isPending}>
            {run.isPending ? t("discovery.searching") : goal.trim() ? t("discovery.search") : t("discovery.searchAuto")}
          </button>
        </div>
        <WhatTheModelSees />
      </form>
      <ErrorBox error={run.error ?? latest.error} />
      {latest.data && <Results d={latest.data} onPickIssue={onPickIssue} />}
    </div>
  );
}
