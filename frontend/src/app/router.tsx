import { BrowserRouter, Route, Routes } from "react-router";
import { AdiPage } from "../analytics/AdiPage";
import { SkillMatrixPage } from "../analytics/SkillMatrixPage";
import { ConversationPage } from "../chat/ConversationPage";
import { TasksPage } from "../issue/TasksPage";
import { LearningPage } from "../learning/LearningPage";
import { ProjectPage } from "../project/ProjectPage";
import { ProjectsPage } from "../project/ProjectsPage";
import { SettingsPage } from "../settings/SettingsPage";
import { WorkspaceHome } from "../workspace/WorkspaceHome";
import { Shell } from "./Shell";

export function AppRouter() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Shell />}>
          <Route index element={<WorkspaceHome />} />
          <Route path="chat/:id" element={<ConversationPage />} />
          <Route path="tasks" element={<TasksPage />} />
          <Route path="projects" element={<ProjectsPage />} />
          <Route path="projects/:id" element={<ProjectPage />} />
          <Route path="learning" element={<LearningPage />} />
          <Route path="skills" element={<SkillMatrixPage />} />
          <Route path="adi" element={<AdiPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<p className="muted pad">Страница не найдена.</p>} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
