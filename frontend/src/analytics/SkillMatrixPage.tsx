import { useQuery } from "@tanstack/react-query";
import { useT, type Key } from "../i18n";
import { api } from "../services/api";
import type { Assessment, MatrixDiscipline } from "../types/api";
import { Badge, ErrorBox, PageTitle, tv, useFormat } from "../ui";

function Dimensions({ a }: { a: Assessment }) {
  const { t } = useT();
  if (a.dimensions.length === 0) return null;
  return (
    <ul className="signals" aria-label={t("esm.dimensions")}>
      {a.dimensions.map((d) => (
        <li key={d.dimension} className="chip">
          {t(`dimension.${d.dimension}` as Key)} {d.score}
        </li>
      ))}
    </ul>
  );
}

function History({ d }: { d: MatrixDiscipline }) {
  const { t } = useT();
  const fmt = useFormat();
  if (d.history.length === 0) return null;
  const skillName = (id: string | null) => (id ? (d.skills.find((s) => s.id === id)?.name ?? id) : t("esm.wholeDiscipline"));
  return (
    <details className="evidence">
      <summary>{t("esm.history", { n: d.history.length })}</summary>
      <ul className="plain">
        {d.history.map((e, i) => (
          <li key={`${e.reference}-${e.dimension}-${e.skill}-${i}`}>
            <div className="row-between">
              <span>{e.title}</span>
              <strong>{e.score}</strong>
            </div>
            <p className="muted small">
              {fmt.date(e.observed_at)} · {t(`source.${e.source}` as Key)} · {t(`dimension.${e.dimension}` as Key)} · {skillName(e.skill)}
            </p>
          </li>
        ))}
      </ul>
    </details>
  );
}

export function SkillMatrixPage() {
  const { t, lang } = useT();
  const m = useQuery({ queryKey: ["skills", lang], queryFn: () => api.skillMatrix(lang) });
  const groups = [
    { id: "fundamental", title: t("esm.fundamental") },
    { id: "engineering", title: t("esm.engineering") },
  ] as const;
  return (
    <div className="page">
      <PageTitle sub={t("esm.subtitle")}>Engineering Skill Matrix</PageTitle>
      <ErrorBox error={m.error} />
      <p className="info-box">{t("esm.note")}</p>
      {groups.map((g) => (
        <section key={g.id}>
          <h2 className="section-title">{g.title}</h2>
          <div className="grid">
            {m.data?.disciplines
              .filter((d) => d.group === g.id)
              .map((d) => (
                <details key={d.id} className="card discipline">
                  <summary>
                    <div>
                      <h3>{d.name}</h3>
                      <p className="muted small">
                        {t("esm.skills", { n: d.skills.length })}
                        {d.evidence_count > 0 && ` · ${t("esm.evidence", { n: d.evidence_count })}`}
                        {d.practice_tasks > 0 && ` · ${t("esm.practice", { n: d.practice_tasks })}`}
                      </p>
                    </div>
                    <div className="score">{d.score ?? "—"}</div>
                  </summary>
                  <div className="discipline-body">
                    <Badge value={d.status} />
                    {d.confidence && <span className="muted small"> {t("adi.confidence", { level: tv(t, "confidence", d.confidence) })}</span>}
                    <Dimensions a={d} />
                  </div>
                  <ul className="plain skills">
                    {d.skills.map((s) => (
                      <li key={s.id}>
                        <span>{s.name}</span>
                        <span className={s.score == null ? "muted small" : "small"} title={tv(t, "status", s.status)}>
                          {s.score ?? t("status.not_assessed")}
                        </span>
                      </li>
                    ))}
                  </ul>
                  <History d={d} />
                </details>
              ))}
          </div>
        </section>
      ))}
    </div>
  );
}
