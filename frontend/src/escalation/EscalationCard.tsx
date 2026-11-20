import { Icon } from "../app/icons";
import { useT, type Key } from "../i18n";
import type { Card } from "../types/api";

type Esc = Extract<Card, { type: "escalation" }>;

export function EscalationCard({ card, busy, onAccept, onReject }: { card: Esc; busy: boolean; onAccept: () => void; onReject: () => void }) {
  const { t } = useT();
  const open = card.status === "proposed";
  return (
    <div className={`escalation ${open ? "" : "resolved"}`} role="group" aria-label={t("escalation.aria")}>
      <span className="escalation-icon">
        <Icon.chevrons />
      </span>
      <div>
        <p>{t("escalation.offer", { to: `L${card.to}`, unlocks: t(`unlocks.${card.to}` as Key) })}</p>
        {open ? (
          <div className="row">
            <button className="primary" disabled={busy} onClick={onAccept}>
              {t("escalation.accept")}
            </button>
            <button className="secondary" disabled={busy} onClick={onReject}>
              {t("escalation.stay", { level: `L${card.from}` })}
            </button>
          </div>
        ) : (
          <p className="muted small">{card.status === "accepted" ? t("escalation.accepted", { level: `L${card.to}` }) : t("escalation.rejected", { level: `L${card.from}` })}</p>
        )}
      </div>
    </div>
  );
}
