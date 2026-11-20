import { BrowserRouter, Route, Routes } from "react-router";
import { Layout } from "../components/Layout";
import { DashboardPage } from "../pages/dashboard/DashboardPage";
import { ProjectPage } from "../pages/projects/ProjectPage";
import { ProjectsPage } from "../pages/projects/ProjectsPage";
import { TasksPage } from "../pages/tasks/TasksPage";
import { SessionPage } from "../pages/workspace/SessionPage";

export function AppRouter() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<DashboardPage />} />
          <Route path="projects" element={<ProjectsPage />} />
          <Route path="projects/:id" element={<ProjectPage />} />
          <Route path="tasks" element={<TasksPage />} />
          <Route path="sessions/:id" element={<SessionPage />} />
          <Route path="*" element={<p className="muted">Страница не найдена.</p>} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
