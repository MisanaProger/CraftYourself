import { LogoMark } from "../app/Logo";
import { EscalationCard } from "../escalation/EscalationCard";
import { useT, type Key } from "../i18n";
import type { Card, Message } from "../types/api";
import { levelName, tv, useFormat } from "../ui";
import { Markdown } from "./Markdown";

interface Props {
  messages: Message[];
  pending: string | null;
  busy: boolean;
  onAccept: (id: string) => void;
  onReject: (id: string) => void;
  onSaveCriteria?: (text: string) => void;
}

type Decision = { kind?: { type?: string }; outcome?: { action?: string }; level?: number } | undefined;

function Sources({ links }: { links: { kind: string; title: string; target: string }[] }) {
  const { t } = useT();
  return (
    <ul className="sources">
      {links.map((l) => (
        <li key={l.target}>
          <span className={`src-kind src-${l.kind}`}>{tv(t, "source", l.kind)}</span>
          {l.target.startsWith("https://") ? (
            <a href={l.target} target="_blank" rel="noreferrer noopener">
              {l.target.replace(/^https:\/\//, "")}
            </a>
          ) : (
            <code>{l.target}</code>
          )}
          <span className="muted small">{l.title}</span>
        </li>
      ))}
    </ul>
  );
}

function RunCard({ card }: { card: Extract<Card, { type: "sandbox_run" }> }) {
  const { t } = useT();
  const fmt = useFormat();
  return (
    <div className={`run-card run-${card.status}`}>
      <span className="run-dot" aria-hidden />
      <div>
        <strong>{tv(t, "runStatus", card.status)}</strong> <code>{card.command}</code>
        <p className="muted small">
          {card.summary ? t("tests.summary", { passed: card.summary.passed, failed: card.summary.failed, ignored: card.summary.ignored }) : null}
          {card.duration_ms != null && ` · ${fmt.duration(card.duration_ms)}`}
        </p>
      </div>
    </div>
  );
}

/** System messages are rendered from their structured data in the UI language. */
function systemText(t: (k: Key, v?: Record<string, string | number>) => string, m: Message): string {
  const meta = (m.meta ?? {}) as { notice?: string; to?: number; decision?: Decision };
  if (meta.notice === "level_raised" && typeof meta.to === "number") {
    return t("chat.levelRaised", { level: `L${meta.to}`, name: levelName(t, meta.to as 0), unlocks: t(`unlocks.${meta.to}` as Key) });
  }
  if (meta.notice === "sandbox_run") return "";
  if (m.cards.some((c) => c.type === "sources")) return t("chat.docsRedirect");
  if (meta.decision?.outcome?.action === "answer") return t("chat.noModel", { level: `L${m.level}`, name: levelName(t, m.level) });
  return m.content;
}

export function MessageList({ messages, pending, busy, onAccept, onReject, onSaveCriteria }: Props) {
  const { t } = useT();
  const fmt = useFormat();
  return (
    <ol className="timeline" aria-live="polite">
      {messages.map((m) => {
        const escalations = m.cards.filter((c) => c.type === "escalation");
        if (m.role === "system" && escalations.length > 0) {
          return (
            <li key={m.id} className="msg-escalation">
              {escalations.map((c) => (
                <EscalationCard key={c.escalation_id} card={c} busy={busy} onAccept={() => onAccept(c.escalation_id)} onReject={() => onReject(c.escalation_id)} />
              ))}
            </li>
          );
        }
        const decision = (m.meta as { decision?: Decision } | null)?.decision;
        const criteria = m.role === "assistant" && decision?.kind?.type === "acceptance_criteria";
        const text = m.role === "system" ? systemText(t, m) : m.content;
        return (
          <li key={m.id} className={`msg msg-${m.role}`}>
            <div className="avatar" aria-hidden>
              {m.role === "user" ? t("chat.you") : <LogoMark size={30} />}
            </div>
            <div className="msg-body">
              <div className="msg-meta">
                <strong>{m.role === "user" ? t("chat.you") : m.role === "assistant" ? "Craft Yourself" : t("chat.system")}</strong>
                <span>{fmt.time(m.created_at)}</span>
                {m.role !== "user" && <span className="lvl">L{m.level}</span>}
              </div>
              <div className="bubble">
                {m.role === "user" ? <p className="pre">{m.content}</p> : text && <Markdown text={text} />}
                {m.cards.map((c, k) =>
                  c.type === "sources" ? (
                    <Sources key={k} links={c.links} />
                  ) : c.type === "policy_notice" ? (
                    <p key={k} className="notice">
                      {c.code ? tv(t, "policy", c.code) : c.text}
                    </p>
                  ) : c.type === "sandbox_run" ? (
                    <RunCard key={k} card={c} />
                  ) : null,
                )}
                {criteria && onSaveCriteria && (
                  <button className="secondary small save-criteria" onClick={() => onSaveCriteria(m.content)}>
                    {t("criteria.saveFromChat")}
                  </button>
                )}
              </div>
            </div>
          </li>
        );
      })}
      {pending && (
        <li className="msg msg-user pending">
          <div className="avatar" aria-hidden>
            {t("chat.you")}
          </div>
          <div className="msg-body">
            <div className="msg-meta">
              <strong>{t("chat.you")}</strong>
              <span>{t("chat.sending")}</span>
            </div>
            <div className="bubble">
              <p className="pre">{pending}</p>
            </div>
          </div>
        </li>
      )}
    </ol>
  );
}
