import { useQuery } from "@tanstack/react-query";
import { useT } from "../i18n";
import { api } from "../services/api";
import { Badge, ErrorBox, PageTitle } from "../ui";

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
                        {d.practice_tasks > 0 && ` · ${t("esm.practice", { n: d.practice_tasks })}`}
                      </p>
                    </div>
                    <div className="score">{d.score ?? "—"}</div>
                  </summary>
                  <Badge value={d.status} />
                  <ul className="plain skills">
                    {d.skills.map((s) => (
                      <li key={s.id}>
                        <span>{s.name}</span>
                        <span className="muted small">{s.score ?? t("status.not_assessed")}</span>
                      </li>
                    ))}
                  </ul>
                </details>
              ))}
          </div>
        </section>
      ))}
    </div>
  );
}
