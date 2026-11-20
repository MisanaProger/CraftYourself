import { BrowserRouter, Route, Routes } from "react-router";
import { AdiPage } from "../analytics/AdiPage";
import { KnowledgePage } from "../analytics/KnowledgePage";
import { SkillMatrixPage } from "../analytics/SkillMatrixPage";
import { ConversationPage } from "../chat/ConversationPage";
import { useT } from "../i18n";
import { TasksPage } from "../issue/TasksPage";
import { LessonPage } from "../learning/LessonPage";
import { ProgramPage } from "../learning/ProgramPage";
import { ProblemsPage } from "../problems/ProblemsPage";
import { ProjectPage } from "../project/ProjectPage";
import { ProjectsPage } from "../project/ProjectsPage";
import { SettingsPage } from "../settings/SettingsPage";
import { HomePage } from "../workspace/HomePage";
import { WorkspacePage } from "../workspace/WorkspacePage";
import { Shell } from "./Shell";

function NotFound() {
  const { t } = useT();
  return <p className="muted pad">{t("common.notFound")}</p>;
}

export function AppRouter() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Shell />}>
          <Route index element={<HomePage />} />
          <Route path="workspace" element={<WorkspacePage />} />
          <Route path="chat/:id" element={<ConversationPage />} />
          <Route path="learning/:program" element={<ProgramPage />} />
          <Route path="learning/:program/:lesson" element={<LessonPage />} />
          <Route path="tasks" element={<TasksPage />} />
          <Route path="problems" element={<ProblemsPage />} />
          <Route path="projects" element={<ProjectsPage />} />
          <Route path="projects/:id" element={<ProjectPage />} />
          <Route path="skills" element={<SkillMatrixPage />} />
          <Route path="adi" element={<AdiPage />} />
          <Route path="knowledge" element={<KnowledgePage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<NotFound />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
