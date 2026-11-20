import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { useT } from "../i18n";
import { api } from "../services/api";
import { Progress } from "../ui";

export function AdiSummary() {
  const { t } = useT();
  const adi = useQuery({ queryKey: ["adi"], queryFn: api.adi });
  const a = adi.data?.aggregate;
  return (
    <Link to="/adi" className="card card-link stat">
      <h2>ADI</h2>
      <div className="big">{a?.value != null ? a.value.toFixed(1) : "—"}</div>
      <p className="muted small">
        {!a || a.issues === 0
          ? t("summary.adiEmpty")
          : `${a.status ? t(`gameStatus.${a.status}` as never) : ""} · ${t("summary.issues", { n: a.issues })}${a.provisional ? ` · ${t("summary.provisional")}` : ""}`}
      </p>
    </Link>
  );
}

export function SkillSummary() {
  const { t, lang } = useT();
  const m = useQuery({ queryKey: ["skills", lang], queryFn: () => api.skillMatrix(lang) });
  const d = m.data?.disciplines ?? [];
  const assessed = d.filter((x) => x.status !== "not_assessed").length;
  return (
    <Link to="/skills" className="card card-link stat">
      <h2>Engineering Skill Matrix</h2>
      <div className="big">
        {assessed}
        <span className="muted">/{d.length || "—"}</span>
      </div>
      <p className="muted small">{t("summary.esm")}</p>
    </Link>
  );
}

export function TaskSummary() {
  const { t } = useT();
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const list = tasks.data ?? [];
  const done = list.filter((x) => x.status === "done").length;
  return (
    <Link to="/tasks" className="card card-link stat">
      <h2>Issues</h2>
      <div className="big">
        {done}
        <span className="muted">/{list.length}</span>
      </div>
      <Progress value={list.length ? (done / list.length) * 100 : 0} label={t("summary.issuesDone")} />
    </Link>
  );
}
