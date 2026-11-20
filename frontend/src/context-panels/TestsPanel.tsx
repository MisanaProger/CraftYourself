import { useQuery } from "@tanstack/react-query";
import { api } from "../services/api";
import type { Task } from "../types/api";
import { Empty, ErrorBox } from "../ui";

export function TestsPanel({ task }: { task: Task | null }) {
  const commands = useQuery({ queryKey: ["projects", task?.project_id, "commands"], queryFn: () => api.projectCommands(task!.project_id), enabled: !!task });
  if (!task) return <Empty>Нет связанной Issue.</Empty>;
  const tests = commands.data?.filter((c) => c.kind !== "build") ?? [];
  return (
    <div>
      <div className="sandbox-note">
        <strong>Sandbox — этап 2.</strong> Запуск тестов в изолированном контейнере ещё не реализован. Пока запускайте их сами в рабочем каталоге.
      </div>
      {task.test_command && (
        <>
          <h3>Тесты задачи</h3>
          <code className="block">{task.test_command}</code>
        </>
      )}
      <h3>Обнаружено адаптерами</h3>
      <ErrorBox error={commands.error} />
      {tests.length === 0 ? (
        <Empty>Команд не найдено.</Empty>
      ) : (
        <ul className="plain">
          {tests.map((c, i) => (
            <li key={i}>
              <span className={`tag tag-${c.kind}`}>{c.kind}</span> <code>{[c.program, ...c.args].join(" ")}</code>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
