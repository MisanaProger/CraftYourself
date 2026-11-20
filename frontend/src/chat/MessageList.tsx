import { LogoMark } from "../app/Logo";
import { EscalationCard } from "../escalation/EscalationCard";
import { formatTime } from "../ui";
import type { Message } from "../types/api";
import { Markdown } from "./Markdown";

interface Props {
  messages: Message[];
  pending: string | null;
  busy: boolean;
  onAccept: (id: string) => void;
  onReject: (id: string) => void;
}

function Sources({ links }: { links: { kind: string; title: string; target: string }[] }) {
  return (
    <ul className="sources">
      {links.map((l) => (
        <li key={l.target}>
          <span className={`src-kind src-${l.kind}`}>{l.kind === "source" ? "код" : l.kind === "curated" ? "автор задачи" : "справочник"}</span>
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

export function MessageList({ messages, pending, busy, onAccept, onReject }: Props) {
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
        return (
          <li key={m.id} className={`msg msg-${m.role}`}>
            <div className="avatar" aria-hidden>
              {m.role === "user" ? "Вы" : <LogoMark size={30} />}
            </div>
            <div className="msg-body">
              <div className="msg-meta">
                <strong>{m.role === "user" ? "Вы" : m.role === "assistant" ? "Craft Yourself" : "Craft Yourself · политика"}</strong>
                <span>{formatTime(m.created_at)}</span>
                {m.role !== "user" && <span className="lvl">L{m.level}</span>}
              </div>
              <div className="bubble">
                {m.role === "user" ? <p className="pre">{m.content}</p> : <Markdown text={m.content} />}
                {m.cards.map((c, k) =>
                  c.type === "sources" ? (
                    <Sources key={k} links={c.links} />
                  ) : c.type === "policy_notice" ? (
                    <p key={k} className="notice">
                      {c.text}
                    </p>
                  ) : null,
                )}
              </div>
            </div>
          </li>
        );
      })}
      {pending && (
        <li className="msg msg-user pending">
          <div className="avatar" aria-hidden>
            Вы
          </div>
          <div className="msg-body">
            <div className="msg-meta">
              <strong>Вы</strong>
              <span>отправка…</span>
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
