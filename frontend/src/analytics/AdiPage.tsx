import { useQuery } from "@tanstack/react-query";
import { api } from "../services/api";
import { Card, Empty, ErrorBox, PageTitle, formatDate } from "../ui";

const CONFIDENCE: Record<string, string> = { low: "низкая", medium: "средняя", high: "высокая" };

export function AdiPage() {
  const adi = useQuery({ queryKey: ["adi"], queryFn: api.adi });
  const d = adi.data;
  return (
    <div className="page">
      <PageTitle sub="Наблюдаемая зависимость от ИИ при выполнении Issue: 0 — самостоятельно, 100 — решение в основном выполнено ИИ. Это не оценка квалификации.">
        ADI Analytics
      </PageTitle>
      <ErrorBox error={adi.error} />
      {d && (
        <>
          <div className="two-col">
            <Card title="Средний ADI с двойным затуханием">
              <div className="big">{d.aggregate.value != null ? d.aggregate.value.toFixed(1) : "—"}</div>
              <p className="muted">
                Учтено Issue: {d.aggregate.issues}
                {d.aggregate.observed_since && ` · с ${formatDate(d.aggregate.observed_since)}`} · уверенность {CONFIDENCE[d.aggregate.confidence]}
              </p>
              {d.aggregate.provisional && d.aggregate.issues > 0 && (
                <p className="warn-box">Предварительно: устойчивый статус появится после {d.params.min_issues_for_status} завершённых Issue.</p>
              )}
              {d.status_label && (
                <div className="game-status">
                  <strong>{d.status_label}</strong>
                  <p className="muted small">{d.disclaimer}</p>
                </div>
              )}
            </Card>
            <Card title="Параметры затухания">
              <p>
                Вес оценки: w = 2<sup>−Δt/T<sub>t</sub></sup> · 2<sup>−Δn/T<sub>n</sub></sup>
              </p>
              <ul className="plain">
                <li>
                  T<sub>t</sub> = {d.params.half_life_days} дней — через столько вес старой оценки уменьшается вдвое
                </li>
                <li>
                  T<sub>n</sub> = {d.params.half_life_issues} Issue — столько новых завершённых Issue уменьшают вес вдвое
                </li>
              </ul>
              <p className="muted small">Предварительные параметры MVP; настраиваются в config.toml, раздел [adi].</p>
            </Card>
          </div>
          <Card title="Оценки по Issue">
            {d.history.length === 0 ? (
              <Empty>Финальный ADI выставляется после завершения Issue.</Empty>
            ) : (
              <ul className="adi-history">
                {d.history.map((h) => (
                  <li key={h.task_id}>
                    <div className="adi-score">{h.score}</div>
                    <div>
                      <strong>{h.task_title ?? h.task_id}</strong>
                      <p className="muted small">
                        {formatDate(h.created_at)} · уверенность {CONFIDENCE[h.confidence] ?? h.confidence} · {h.scoring_version}
                      </p>
                      <p className="small">{h.explanation}</p>
                      {h.factors.length > 0 && (
                        <ul className="factors">
                          {h.factors.map((f) => (
                            <li key={f.description} className={f.points > 0 ? "up" : "down"}>
                              {f.points > 0 ? "+" : ""}
                              {f.points.toFixed(1)} {f.description}
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
