import { useQuery } from "@tanstack/react-query";
import { api } from "../services/api";
import { Badge, ErrorBox, PageTitle } from "../ui";

export function SkillMatrixPage() {
  const m = useQuery({ queryKey: ["skills"], queryFn: api.skillMatrix });
  const groups = [
    { id: "fundamental", title: "Фундаментальные дисциплины" },
    { id: "engineering", title: "Инженерные дисциплины" },
  ] as const;
  return (
    <div className="page">
      <PageTitle sub="Развитие по отдельным дисциплинам. Независимо от ADI и не сводится к одному рейтингу.">Engineering Skill Matrix</PageTitle>
      <ErrorBox error={m.error} />
      {m.data && <p className="info-box">{m.data.note}</p>}
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
                        {d.skills.length} навыков{d.practice_tasks > 0 && ` · практика: ${d.practice_tasks} Issue`}
                      </p>
                    </div>
                    <div className="score">{d.score ?? "—"}</div>
                  </summary>
                  <Badge value={d.status} />
                  <ul className="plain skills">
                    {d.skills.map((s) => (
                      <li key={s.id}>
                        <span>{s.name}</span>
                        <span className="muted small">{s.score ?? "не оценено"}</span>
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
