import { Icon } from "../app/icons";
import type { Card } from "../types/api";

type Esc = Extract<Card, { type: "escalation" }>;

export function EscalationCard({ card, busy, onAccept, onReject }: { card: Esc; busy: boolean; onAccept: () => void; onReject: () => void }) {
  const open = card.status === "proposed";
  return (
    <div className={`escalation ${open ? "" : "resolved"}`} role="group" aria-label="Предложение повысить уровень помощи">
      <span className="escalation-icon">
        <Icon.chevrons />
      </span>
      <div>
        <p>
          Могу перейти на уровень <strong>L{card.to}</strong>: станут доступны {card.unlocks}. Перейти на L{card.to}?
        </p>
        {open ? (
          <div className="row">
            <button className="primary" disabled={busy} onClick={onAccept}>
              Повысить уровень
            </button>
            <button className="secondary" disabled={busy} onClick={onReject}>
              Остаться на L{card.from}
            </button>
          </div>
        ) : (
          <p className="muted small">{card.status === "accepted" ? `Уровень повышен до L${card.to}` : `Остались на L${card.from}`}</p>
        )}
      </div>
    </div>
  );
}
