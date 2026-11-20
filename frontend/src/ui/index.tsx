import type { ReactNode } from "react";
import { useT, type Key, type T } from "../i18n";
import { LEVEL_CODES, type Level } from "../types/api";

/** Translates `prefix.value`, falling back to the raw value. */
export function tv(t: T, prefix: string, value: string) {
  const key = `${prefix}.${value}` as Key;
  const s = t(key);
  return s === key ? value : s;
}

export function Badge({ value }: { value: string }) {
  const { t } = useT();
  return (
    <span className={`badge badge-${value}`}>
      <i aria-hidden />
      {tv(t, "status", value)}
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

export function levelName(t: T, level: Level) {
  return t(`level.${level}` as Key);
}

export function levelLabel(t: T, level: Level) {
  return `${LEVEL_CODES[level]} · ${levelName(t, level)}`;
}

export const shortHash = (hash: string) => hash.slice(0, 8);
export const shortId = (id: string) => id.slice(0, 8).toUpperCase();

export function useFormat() {
  const { lang } = useT();
  const locale = lang === "uz" ? "uz-Latn-UZ" : lang === "en" ? "en-GB" : "ru-RU";
  return {
    time: (iso: string) => new Date(iso).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" }),
    date: (iso: string) => new Date(iso).toLocaleString(locale, { dateStyle: "short", timeStyle: "short" }),
    duration: (ms: number | null) => (ms == null ? "" : ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`),
  };
}

export function PageTitle({ children, sub, actions }: { children: ReactNode; sub?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="page-title row-between">
      <div>
        <h1>{children}</h1>
        {sub && <p className="muted">{sub}</p>}
      </div>
      {actions}
    </header>
  );
}

export function Progress({ value, label }: { value: number; label: string }) {
  const pct = Math.max(0, Math.min(100, Math.round(value)));
  return (
    <div className="meter" role="meter" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} aria-label={label}>
      <span style={{ width: `${pct}%` }} />
    </div>
  );
}
