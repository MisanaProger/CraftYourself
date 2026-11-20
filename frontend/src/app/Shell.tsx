import { useQuery } from "@tanstack/react-query";
import { NavLink, Outlet } from "react-router";
import { useLiveEvents } from "../hooks/useLiveEvents";
import { api } from "../services/api";
import { Icon } from "./icons";
import { Logo } from "./Logo";

const NAV = [
  { to: "/", label: "Рабочее пространство", icon: Icon.chat, end: true },
  { to: "/tasks", label: "Задачи", icon: Icon.tasks },
  { to: "/projects", label: "Проекты", icon: Icon.projects },
  { to: "/learning", label: "Обучение", icon: Icon.learning },
  { to: "/skills", label: "Skill Matrix", icon: Icon.skills },
  { to: "/adi", label: "ADI Analytics", icon: Icon.adi },
  { to: "/settings", label: "Настройки", icon: Icon.settings },
];

export function Shell() {
  useLiveEvents();
  const privacy = useQuery({ queryKey: ["privacy"], queryFn: api.privacy });
  const remote = privacy.data?.llm?.remote;
  return (
    <div className="shell">
      <aside className="sidebar">
        <NavLink to="/" className="brand" aria-label="Craft Yourself — на главную">
          <Logo />
        </NavLink>
        <p className="tagline">Build skills. Not dependencies.</p>
        <nav className="nav">
          {NAV.map(({ to, label, icon: I, end }) => (
            <NavLink key={to} to={to} end={end}>
              <I />
              <span>{label}</span>
            </NavLink>
          ))}
        </nav>
        <div className={`locality ${remote ? "remote" : ""}`}>
          <i aria-hidden />
          <div>
            <strong>{remote ? "ВНЕШНИЙ LLM" : "LOCAL"}</strong>
            <span>{remote ? "контекст уходит провайдеру" : "Данные на устройстве"}</span>
          </div>
        </div>
      </aside>
      <main className="main">
        <Outlet />
      </main>
    </div>
  );
}
