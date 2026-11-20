import type { JSX } from "react";
import { useQuery } from "@tanstack/react-query";
import { NavLink, Outlet } from "react-router";
import { useLiveEvents } from "../hooks/useLiveEvents";
import { LangSwitch, useT, type Key } from "../i18n";
import { api } from "../services/api";
import { Icon } from "./icons";
import { Logo } from "./Logo";

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
  return (
    <div className="shell">
      <aside className="sidebar">
        <NavLink to="/" className="brand" aria-label={t("nav.homeAria")}>
          <Logo />
        </NavLink>
        <p className="tagline">Build skills. Not dependencies.</p>
        <nav className="nav">
          {NAV.map(({ to, key, icon: I, end }) => (
            <NavLink key={to} to={to} end={end}>
              <I />
              <span>{t(key)}</span>
            </NavLink>
          ))}
        </nav>
        <div className="sidebar-foot">
          <LangSwitch />
          <div className={`locality ${remote ? "remote" : ""}`}>
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
