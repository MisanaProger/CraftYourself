import type { ReactNode } from "react";

const STATUS_LABELS: Record<string, string> = {
  open: "открыта",
  "in-progress": "в работе",
  done: "выполнена",
  archived: "в архиве",
  active: "активна",
  completed: "завершена",
  abandoned: "прервана",
};

export function Badge({ value }: { value: string }) {
  return <span className={`badge badge-${value}`}>{STATUS_LABELS[value] ?? value}</span>;
}

export function ErrorBox({ error }: { error: unknown }) {
  if (!error) return null;
  return <div className="error">{error instanceof Error ? error.message : String(error)}</div>;
}

export function Card({ title, actions, children }: { title?: ReactNode; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="card">
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

export function shortHash(hash: string) {
  return hash.slice(0, 8);
}

export function formatDate(iso: string) {
  return new Date(iso).toLocaleString("ru-RU", { dateStyle: "short", timeStyle: "short" });
}
