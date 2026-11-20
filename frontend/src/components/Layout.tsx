import { NavLink, Outlet } from "react-router";
import { useLiveEvents } from "../hooks/useLiveEvents";

export function Layout() {
  useLiveEvents();
  return (
    <div className="shell">
      <header className="topbar">
        <NavLink to="/" className="brand">
          Engineer Craft
        </NavLink>
        <nav>
          <NavLink to="/" end>
            Обзор
          </NavLink>
          <NavLink to="/projects">Проекты</NavLink>
          <NavLink to="/tasks">Задачи</NavLink>
        </nav>
      </header>
      <main className="content">
        <Outlet />
      </main>
    </div>
  );
}
