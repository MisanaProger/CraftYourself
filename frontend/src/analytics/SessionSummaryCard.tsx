import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { useT, type Key } from "../i18n";
import { api } from "../services/api";
import type { SessionSummary, Signal } from "../types/api";
import { Badge, ErrorBox, levelLabel, tv, useFormat } from "../ui";

const GOOD: Signal[] = ["worked_independently", "declined_help", "used_documentation", "tests_green", "tests_recovered", "criteria_met", "own_commits"];
const WARN: Signal[] = ["heavy_help", "tests_red_at_finish", "criteria_open", "no_commits", "abandoned"];

export function SignalChips({ signals }: { signals: Signal[] }) {
  const { t } = useT();
  if (signals.length === 0) return null;
  return (
    <ul className="signals" aria-label={t("summary.signals")}>
      {signals.map((s) => (
        <li key={s} className={`chip ${GOOD.includes(s) ? "good" : WARN.includes(s) ? "warn" : ""}`}>
          {t(`signal.${s}` as Key)}
        </li>
      ))}
    </ul>
  );
}

/** One session's summary: counts and signals, nothing from the messages. */
export function SessionSummaryView({ summary: s, heading = true }: { summary: SessionSummary; heading?: boolean }) {
  const { t } = useT();
  const fmt = useFormat();
  const solution = s.help.answers_by_level.reduce((a, b) => a + b, 0);
  return (
    <div className="session-summary">
      {heading && (
        <div className="row-between">
          <strong>
            {s.conversation_id ? <Link to={`/chat/${s.conversation_id}`}>{s.task_title}</Link> : s.task_title}
          </strong>
          <Badge value={s.outcome} />
        </div>
      )}
      <p className="muted small">
        {fmt.date(s.finished_at)} · {t("summary.duration", { n: s.duration_minutes })} · {tv(t, "taskType", s.task_type)}
      </p>
      <SignalChips signals={s.signals} />
      <dl className="kv">
        <dt>{t("summary.help")}</dt>
        <dd>{t("summary.helpValue", { level: levelLabel(t, s.help.max_level), answers: solution })}</dd>
        <dt>{t("summary.tests")}</dt>
        <dd>
          {s.runs.test_runs === 0
            ? t("summary.noRuns")
            : t("summary.testsValue", { runs: s.runs.test_runs, passed: s.runs.passed, failed: s.runs.failed })}
        </dd>
        {s.criteria_total > 0 && (
          <>
            <dt>{t("summary.criteria")}</dt>
            <dd>{t("summary.criteriaValue", { done: s.criteria_done, total: s.criteria_total })}</dd>
          </>
        )}
        <dt>{t("summary.commits")}</dt>
        <dd>{s.commits}</dd>
        {s.commit_quality != null && (
          <>
            <dt>{t("summary.commitQuality")}</dt>
            <dd>{s.commit_quality}/100</dd>
          </>
        )}
        {s.adi != null && (
          <>
            <dt>ADI</dt>
            <dd>{s.adi}</dd>
          </>
        )}
      </dl>
    </div>
  );
}

/** The summary of a finished session, in the Issue panel. */
export function SessionSummaryPanel({ sessionId }: { sessionId: string }) {
  const { t } = useT();
  const q = useQuery({ queryKey: ["summary", sessionId], queryFn: () => api.sessionSummary(sessionId) });
  return (
    <section className="summary-panel">
      <h3>{t("summary.title")}</h3>
      <ErrorBox error={q.error} />
      {q.data && <SessionSummaryView summary={q.data} heading={false} />}
      <p className="hint">
        {t("summary.hint")} <Link to="/knowledge">{t("summary.profileLink")}</Link>
      </p>
    </section>
  );
}
