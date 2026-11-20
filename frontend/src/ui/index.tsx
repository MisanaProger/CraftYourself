import type { ReactNode } from "react";
import { LEVELS, type Level } from "../types/api";

const STATUS: Record<string, string> = {
  open: "Открыта",
  "in-progress": "В работе",
  done: "Выполнена",
  archived: "В архиве",
  active: "В работе",
  completed: "Завершена",
  abandoned: "Прервана",
  not_assessed: "Не оценено",
  insufficient_evidence: "Мало данных",
  provisional: "Предварительно",
  established: "Подтверждено",
  needs_reassessment: "Нужна перепроверка",
};

export function Badge({ value }: { value: string }) {
  return (
    <span className={`badge badge-${value}`}>
      <i aria-hidden />
      {STATUS[value] ?? value}
    </span>
  );
}

export function ErrorBox({ error }: { error: unknown }) {
  if (!error) return null;
  return (
    <div className="error" role="alert">
      {error instanceof Error ? error.message : String(error)}
    </div>
  );
}

export function Card({ title, actions, children, className = "" }: { title?: ReactNode; actions?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={`card ${className}`}>
      {(title || actions) && (
        <div className="card-head">
          {title && <h2>{title}</h2>}
          {actions}
        </div>
      )}
      {children}
    </section>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return <p className="muted">{children}</p>;
}

export function levelLabel(level: Level) {
  const l = LEVELS[level];
  return `${l.code} · ${l.name}`;
}

export const shortHash = (hash: string) => hash.slice(0, 8);
export const shortId = (id: string) => id.slice(0, 8).toUpperCase();
export const formatTime = (iso: string) => new Date(iso).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
export const formatDate = (iso: string) => new Date(iso).toLocaleString("ru-RU", { dateStyle: "short", timeStyle: "short" });

export function PageTitle({ children, sub }: { children: ReactNode; sub?: ReactNode }) {
  return (
    <header className="page-title">
      <h1>{children}</h1>
      {sub && <p className="muted">{sub}</p>}
    </header>
  );
}
