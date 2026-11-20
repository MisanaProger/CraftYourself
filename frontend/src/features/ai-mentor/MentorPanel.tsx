import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { ErrorBox, formatDate } from "../../components/ui";
import { api } from "../../services/api";
import { HELP_LEVELS, type HelpLevel, type MentorAnswer } from "../../types/api";

const KIND_LABELS: Record<string, string> = {
  api_reference: "справочный вопрос об API",
  conceptual: "концептуальный вопрос",
  mentoring: "вопрос по ходу работы",
};

function LinkTarget({ target }: { target: string }) {
  return target.startsWith("https://") ? (
    <a href={target} target="_blank" rel="noreferrer noopener">
      {target}
    </a>
  ) : (
    <code>{target}</code>
  );
}

function Answer({ a }: { a: MentorAnswer }) {
  const levelLabel = HELP_LEVELS[a.help.granted].label;
  return (
    <div className={`answer ${a.forward_to_model ? "" : "answer-redirect"}`}>
      <p>
        <strong>{KIND_LABELS[a.kind.type]}</strong> · уровень {a.help.granted} ({levelLabel.toLowerCase()})
      </p>
      {a.help.limited_by && <p className="muted small">Ограничено: {a.help.limited_by}</p>}
      <p>{a.note}</p>
      {a.links.length > 0 && (
        <ul className="links">
          {a.links.map((l) => (
            <li key={l.target}>
              <LinkTarget target={l.target} />
              <span className="muted small">{l.title}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function MentorPanel({ sessionId, maxLevel, active }: { sessionId: string; maxLevel: HelpLevel; active: boolean }) {
  const qc = useQueryClient();
  const [question, setQuestion] = useState("");
  const [excerpt, setExcerpt] = useState("");
  const [level, setLevel] = useState<HelpLevel>(1);
  const history = useQuery({ queryKey: ["session", sessionId, "mentor"], queryFn: () => api.mentorHistory(sessionId) });
  const ask = useMutation({
    mutationFn: () => api.ask(sessionId, question, level, excerpt),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["session", sessionId, "mentor"] }),
  });

  return (
    <>
      <form
        className="stack"
        onSubmit={(e) => {
          e.preventDefault();
          if (question.trim()) ask.mutate();
        }}
      >
        <textarea
          rows={3}
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          placeholder="Опишите, что вы уже выяснили и где застряли"
          aria-label="Вопрос ментору"
          disabled={!active}
        />
        <details>
          <summary>Объяснить термин из документации</summary>
          <textarea rows={3} value={excerpt} onChange={(e) => setExcerpt(e.target.value)} placeholder="Вставьте короткий фрагмент (до 1500 символов)" aria-label="Фрагмент документации" />
        </details>
        <div className="row">
          <select value={level} onChange={(e) => setLevel(Number(e.target.value) as HelpLevel)} aria-label="Уровень помощи">
            {HELP_LEVELS.map((h) => (
              <option key={h.level} value={h.level} disabled={h.level > maxLevel}>
                {h.level} — {h.label}
              </option>
            ))}
          </select>
          <button disabled={!active || !question.trim() || ask.isPending}>Спросить</button>
        </div>
      </form>
      <ErrorBox error={ask.error} />
      {ask.data && <Answer a={ask.data} />}
      {(history.data?.length ?? 0) > 0 && (
        <details className="history">
          <summary>История обращений ({history.data!.length})</summary>
          <ul className="list compact">
            {history.data!.map((e) => (
              <li key={e.id}>
                <span className="muted small">{formatDate(e.at)}</span>
                <span>{String(e.payload.question ?? "")}</span>
                <span className="muted small">
                  {String(e.payload.requested_level)}→{String(e.payload.granted_level)} · {String(e.payload.action)}
                </span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </>
  );
}
