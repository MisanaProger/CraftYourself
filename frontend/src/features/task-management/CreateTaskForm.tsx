import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { ErrorBox } from "../../components/ui";
import { api } from "../../services/api";
import type { Difficulty, TaskType } from "../../types/api";

export function CreateTaskForm({ projectId }: { projectId: string }) {
  const qc = useQueryClient();
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [taskType, setTaskType] = useState<TaskType>("bug-fix");
  const [difficulty, setDifficulty] = useState<Difficulty>("beginner");
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
        create.mutate({ project_id: projectId, title, description, task_type: taskType, difficulty });
      }}
    >
      <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Название" aria-label="Название" />
      <textarea
        value={description}
        onChange={(e) => setDescription(e.target.value)}
        placeholder="Что нужно сделать и как понять, что сделано"
        aria-label="Описание"
        rows={3}
      />
      <div className="row">
        <select value={taskType} onChange={(e) => setTaskType(e.target.value as TaskType)} aria-label="Тип">
          <option value="bug-fix">Исправление ошибки</option>
          <option value="feature">Функция</option>
          <option value="refactoring">Рефакторинг</option>
          <option value="documentation">Документация</option>
          <option value="performance">Производительность</option>
          <option value="testing">Тестирование</option>
        </select>
        <select value={difficulty} onChange={(e) => setDifficulty(e.target.value as Difficulty)} aria-label="Сложность">
          <option value="beginner">Начальная</option>
          <option value="intermediate">Средняя</option>
          <option value="advanced">Высокая</option>
        </select>
        <button disabled={!title.trim() || create.isPending}>Создать</button>
      </div>
      <ErrorBox error={create.error} />
    </form>
  );
}
