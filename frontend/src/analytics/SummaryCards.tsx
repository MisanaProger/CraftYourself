import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { api } from "../services/api";

export function AdiSummary() {
  const adi = useQuery({ queryKey: ["adi"], queryFn: api.adi });
  const a = adi.data?.aggregate;
  return (
    <Link to="/adi" className="card card-link stat">
      <h2>ADI</h2>
      <div className="big">{a?.value != null ? a.value.toFixed(1) : "—"}</div>
      <p className="muted small">
        {!a || a.issues === 0
          ? "Появится после первой завершённой Issue"
          : `${adi.data?.status_label ?? ""} · ${a.issues} Issue${a.provisional ? " · предварительно" : ""}`}
      </p>
    </Link>
  );
}

export function SkillSummary() {
  const m = useQuery({ queryKey: ["skills"], queryFn: api.skillMatrix });
  const d = m.data?.disciplines ?? [];
  const assessed = d.filter((x) => x.status !== "not_assessed").length;
  return (
    <Link to="/skills" className="card card-link stat">
      <h2>Engineering Skill Matrix</h2>
      <div className="big">
        {assessed}
        <span className="muted">/{d.length || "—"}</span>
      </div>
      <p className="muted small">дисциплин оценено · отсутствие данных ≠ ноль</p>
    </Link>
  );
}

export function TaskSummary() {
  const tasks = useQuery({ queryKey: ["tasks"], queryFn: () => api.tasks() });
  const t = tasks.data ?? [];
  const done = t.filter((x) => x.status === "done").length;
  const pct = t.length ? Math.round((done / t.length) * 100) : 0;
  return (
    <Link to="/tasks" className="card card-link stat">
      <h2>Issues</h2>
      <div className="big">
        {done}
        <span className="muted">/{t.length}</span>
      </div>
      <div className="meter" role="meter" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} aria-label="Завершено Issue">
        <span style={{ width: `${pct}%` }} />
      </div>
    </Link>
  );
}
