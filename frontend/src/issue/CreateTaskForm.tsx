import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../services/api";
import type { Difficulty, TaskType } from "../types/api";
import { ErrorBox } from "../ui";
import { TYPE_LABELS } from "./TaskTable";

export function CreateTaskForm({ projectId }: { projectId: string }) {
  const qc = useQueryClient();
  const matrix = useQuery({ queryKey: ["skills"], queryFn: api.skillMatrix });
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
      <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Название Issue" aria-label="Название" />
      <textarea value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Проблема, ожидаемое и фактическое поведение" aria-label="Описание" rows={3} />
      <div className="row">
        <select value={taskType} onChange={(e) => setTaskType(e.target.value as TaskType)} aria-label="Тип">
          {Object.entries(TYPE_LABELS).map(([v, l]) => (
            <option key={v} value={v}>
              {l}
            </option>
          ))}
        </select>
        <select value={difficulty} onChange={(e) => setDifficulty(e.target.value as Difficulty)} aria-label="Сложность">
          <option value="beginner">Начальная</option>
          <option value="intermediate">Средняя</option>
          <option value="advanced">Высокая</option>
        </select>
        <select value={discipline} onChange={(e) => setDiscipline(e.target.value)} aria-label="Дисциплина">
          <option value="">Дисциплина…</option>
          {matrix.data?.disciplines.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name}
            </option>
          ))}
        </select>
        <button className="primary" disabled={!title.trim() || create.isPending}>
          Создать
        </button>
      </div>
      <ErrorBox error={create.error} />
    </form>
  );
}
