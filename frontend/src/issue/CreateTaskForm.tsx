import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../services/api";
import type { Difficulty, TaskType } from "../types/api";
import { ErrorBox } from "../ui";
import { TASK_TYPES } from "./TaskTable";
import { useT } from "../i18n";
import { tv } from "../ui";

export function CreateTaskForm({ projectId }: { projectId: string }) {
  const { t, lang } = useT();
  const qc = useQueryClient();
  const matrix = useQuery({ queryKey: ["skills", lang], queryFn: () => api.skillMatrix(lang) });
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [taskType, setTaskType] = useState<TaskType>("bug-fix");
  const [difficulty, setDifficulty] = useState<Difficulty>("beginner");
  const [discipline, setDiscipline] = useState("");
  const create = useMutation({
    mutationFn: api.createTask,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
      setTitle("");
      setDescription("");
    },
  });
  return (
    <form
      className="stack"
      onSubmit={(e) => {
        e.preventDefault();
        create.mutate({ project_id: projectId, title, description, task_type: taskType, difficulty, disciplines: discipline ? [discipline] : [] });
      }}
    >
      <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t("newTask.title")} aria-label={t("newTask.title")} />
      <textarea value={description} onChange={(e) => setDescription(e.target.value)} placeholder={t("newTask.description")} aria-label={t("newTask.description")} rows={3} />
      <div className="row">
        <select value={taskType} onChange={(e) => setTaskType(e.target.value as TaskType)} aria-label={t("tasks.type")}>
          {TASK_TYPES.map((v) => (
            <option key={v} value={v}>
              {tv(t, "taskType", v)}
            </option>
          ))}
        </select>
        <select value={difficulty} onChange={(e) => setDifficulty(e.target.value as Difficulty)} aria-label={t("tasks.difficulty")}>
          <option value="beginner">{tv(t, "difficulty", "beginner")}</option>
          <option value="intermediate">{tv(t, "difficulty", "intermediate")}</option>
          <option value="advanced">{tv(t, "difficulty", "advanced")}</option>
        </select>
        <select value={discipline} onChange={(e) => setDiscipline(e.target.value)} aria-label={t("newTask.discipline")}>
          <option value="">{t("newTask.discipline")}…</option>
          {matrix.data?.disciplines.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name}
            </option>
          ))}
        </select>
        <button className="primary" disabled={!title.trim() || create.isPending}>
          {t("common.create")}
        </button>
      </div>
      <ErrorBox error={create.error} />
    </form>
  );
}
