import { useQuery } from "@tanstack/react-query";
import { useT, type Key } from "../i18n";
import { api } from "../services/api";
import type { TopicKnowledge } from "../types/api";
import { Card, Empty, ErrorBox, PageTitle, tv, useFormat } from "../ui";
import { SessionSummaryView } from "./SessionSummaryCard";

function topicLabel(t: ReturnType<typeof useT>["t"], topic: TopicKnowledge) {
  return topic.kind === "task_type" ? tv(t, "taskType", topic.id) : topic.label;
}

export function KnowledgePage() {
  const { t, lang } = useT();
  const fmt = useFormat();
  const q = useQuery({ queryKey: ["knowledge", lang], queryFn: () => api.knowledgeProfile(lang) });
  const p = q.data;
  return (
    <div className="page">
      <PageTitle sub={t("knowledge.subtitle")}>{t("knowledge.title")}</PageTitle>
      <ErrorBox error={q.error} />
      <p className="info-box">{t("knowledge.note")}</p>
      {p && p.sessions === 0 && <Empty>{t("knowledge.empty")}</Empty>}
      {p && p.sessions > 0 && (
        <>
          <div className="three-col">
            <Card title={t("knowledge.sessions")}>
              <div className="big">{p.sessions}</div>
            </Card>
            <Card title={t("knowledge.completed")}>
              <div className="big">{p.completed}</div>
            </Card>
            <Card title={t("knowledge.abandoned")}>
              <div className="big">{p.abandoned}</div>
            </Card>
          </div>
          <Card title={t("knowledge.topics")}>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>{t("knowledge.topic")}</th>
                    <th>{t("knowledge.standing")}</th>
                    <th>{t("knowledge.done")}</th>
                    <th>{t("knowledge.independent")}</th>
                    <th>{t("knowledge.avgAdi")}</th>
                    <th>{t("knowledge.confidence")}</th>
                    <th>{t("knowledge.last")}</th>
                  </tr>
                </thead>
                <tbody>
                  {p.topics.map((topic) => (
                    <tr key={`${topic.kind}:${topic.id}`}>
                      <td>
                        {topicLabel(t, topic)} <span className="muted small">· {t(`topicKind.${topic.kind}` as Key)}</span>
                      </td>
                      <td>
                        <span className={`chip standing-${topic.standing}`}>{t(`standing.${topic.standing}` as Key)}</span>
                      </td>
                      <td>
                        {topic.completed} / {topic.abandoned}
                      </td>
                      <td>
                        {topic.independent} / {topic.completed}
                      </td>
                      <td>{topic.average_adi ?? "—"}</td>
                      <td>{tv(t, "confidence", topic.confidence)}</td>
                      <td className="muted small">{fmt.date(topic.last_practised)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Card>
          <h2 className="section-title">{t("knowledge.recent")}</h2>
          <div className="grid">
            {p.recent.map((s) => (
              <div key={s.session_id} className="card">
                <SessionSummaryView summary={s} />
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
