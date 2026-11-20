import { useQuery } from "@tanstack/react-query";
import { useT, type Key } from "../i18n";
import { api } from "../services/api";
import { Card, Empty, ErrorBox, PageTitle, tv, useFormat } from "../ui";

export function AdiPage() {
  const { t } = useT();
  const fmt = useFormat();
  const adi = useQuery({ queryKey: ["adi"], queryFn: api.adi });
  const d = adi.data;
  return (
    <div className="page">
      <PageTitle sub={t("adi.subtitle")}>ADI Analytics</PageTitle>
      <ErrorBox error={adi.error} />
      {d && (
        <>
          <div className="two-col">
            <Card title={t("adi.average")}>
              <div className="big">{d.aggregate.value != null ? d.aggregate.value.toFixed(1) : "—"}</div>
              <p className="muted">
                {t("adi.counted", { n: d.aggregate.issues })}
                {d.aggregate.observed_since && ` · ${t("adi.since", { date: fmt.date(d.aggregate.observed_since) })}`} ·{" "}
                {t("adi.confidence", { level: tv(t, "confidence", d.aggregate.confidence) })}
              </p>
              {d.aggregate.provisional && d.aggregate.issues > 0 && <p className="warn-box">{t("adi.provisional", { n: d.params.min_issues_for_status })}</p>}
              {d.aggregate.status && (
                <div className="game-status">
                  <strong>{t(`gameStatus.${d.aggregate.status}` as Key)}</strong>
                  <p className="muted small">{t("adi.disclaimer")}</p>
                </div>
              )}
            </Card>
            <Card title={t("adi.decay")}>
              <p>
                {t("adi.weight")}: w = 2<sup>−Δt/T<sub>t</sub></sup> · 2<sup>−Δn/T<sub>n</sub></sup>
              </p>
              <ul className="plain">
                <li>
                  T<sub>t</sub> = {t("adi.tt", { n: d.params.half_life_days })}
                </li>
                <li>
                  T<sub>n</sub> = {t("adi.tn", { n: d.params.half_life_issues })}
                </li>
              </ul>
              <p className="muted small">{t("adi.paramsNote")}</p>
            </Card>
          </div>
          <Card title={t("adi.perIssue")}>
            {d.history.length === 0 ? (
              <Empty>{t("adi.empty")}</Empty>
            ) : (
              <ul className="adi-history">
                {d.history.map((h) => (
                  <li key={h.task_id}>
                    <div className="adi-score">{h.score}</div>
                    <div>
                      <strong>{h.task_title ?? h.task_id}</strong>
                      <p className="muted small">
                        {fmt.date(h.created_at)} · {t("adi.confidence", { level: tv(t, "confidence", h.confidence) })} · {h.scoring_version}
                      </p>
                      <p className="small">{t("adi.explanation")}</p>
                      {h.factors.length > 0 && (
                        <ul className="factors">
                          {h.factors.map((f) => (
                            <li key={f.code ?? f.description} className={f.points > 0 ? "up" : "down"}>
                              {f.points > 0 ? "+" : ""}
                              {f.points.toFixed(1)} {f.code ? t(`factor.${f.code}` as Key, { n: f.count }) : f.description}
                            </li>
                          ))}
                        </ul>
                      )}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </Card>
        </>
      )}
    </div>
  );
}
