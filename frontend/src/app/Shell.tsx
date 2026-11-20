import { useCallback, useEffect, useState, type JSX } from "react";
import { useQuery } from "@tanstack/react-query";
import { NavLink, Outlet } from "react-router";
import { useLiveEvents } from "../hooks/useLiveEvents";
import { LangSwitch, useT, type Key } from "../i18n";
import { api } from "../services/api";
import { Icon } from "./icons";
import { Logo, LogoMark } from "./Logo";

const COLLAPSED_KEY = "craft-sidebar-collapsed";

function initialCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSED_KEY) === "1";
  } catch {
    return false;
  }
}

/** Sidebar collapsed to icons; remembered per browser, toggled with Ctrl/⌘+B. */
function useCollapsed() {
  const [collapsed, setCollapsed] = useState(initialCollapsed);
  useEffect(() => {
    try {
      localStorage.setItem(COLLAPSED_KEY, collapsed ? "1" : "0");
    } catch {
      /* storage unavailable */
    }
  }, [collapsed]);
  const toggle = useCallback(() => setCollapsed((c) => !c), []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey || e.key.toLowerCase() !== "b") return;
      const el = e.target as HTMLElement | null;
      if (el && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName))) return;
      e.preventDefault();
      toggle();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle]);
  return { collapsed, toggle };
}

const NAV: { to: string; key: Key; icon: () => JSX.Element; end?: boolean }[] = [
  { to: "/", key: "nav.home", icon: Icon.home, end: true },
  { to: "/workspace", key: "nav.workspace", icon: Icon.chat },
  { to: "/tasks", key: "nav.tasks", icon: Icon.tasks },
  { to: "/projects", key: "nav.projects", icon: Icon.projects },
  { to: "/skills", key: "nav.skills", icon: Icon.skills },
  { to: "/adi", key: "nav.adi", icon: Icon.adi },
  { to: "/settings", key: "nav.settings", icon: Icon.settings },
];

export function Shell() {
  useLiveEvents();
  const { t } = useT();
  const privacy = useQuery({ queryKey: ["privacy"], queryFn: api.privacy });
  const remote = privacy.data?.llm?.remote;
  const { collapsed, toggle } = useCollapsed();
  const toggleLabel = collapsed ? t("nav.expand") : t("nav.collapse");
  return (
    <div className={`shell ${collapsed ? "collapsed" : ""}`}>
      <aside className="sidebar" id="sidebar">
        <div className="sidebar-head">
          <NavLink to="/" className="brand" aria-label={t("nav.homeAria")}>
            {collapsed ? <LogoMark size={32} /> : <Logo />}
          </NavLink>
          <button
            type="button"
            className="icon-btn sidebar-toggle"
            onClick={toggle}
            aria-expanded={!collapsed}
            aria-controls="sidebar-nav"
            aria-label={toggleLabel}
            title={`${toggleLabel} (Ctrl+B)`}
          >
            <Icon.panel open={!collapsed} />
          </button>
        </div>
        <p className="tagline">Build skills. Not dependencies.</p>
        <nav className="nav" id="sidebar-nav">
          {NAV.map(({ to, key, icon: I, end }) => (
            <NavLink key={to} to={to} end={end} title={collapsed ? t(key) : undefined} aria-label={collapsed ? t(key) : undefined}>
              <I />
              <span>{t(key)}</span>
            </NavLink>
          ))}
        </nav>
        <div className="sidebar-foot">
          <LangSwitch />
          <div
            className={`locality ${remote ? "remote" : ""}`}
            title={collapsed ? (remote ? `${t("locality.remote")} — ${t("locality.remoteHint")}` : t("locality.local")) : undefined}
          >
            <i aria-hidden />
            <div>
              <strong>{remote ? t("locality.remote") : "LOCAL"}</strong>
              <span>{remote ? t("locality.remoteHint") : t("locality.local")}</span>
            </div>
          </div>
        </div>
      </aside>
      <main className="main">
        <Outlet />
      </main>
    </div>
  );
}
