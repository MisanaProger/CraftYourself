import { useQuery } from "@tanstack/react-query";
import { useParams } from "react-router";
import { Card, Empty, ErrorBox } from "../../components/ui";
import { CreateTaskForm } from "../../features/task-management/CreateTaskForm";
import { ImportExerciseForm } from "../../features/task-management/ImportExerciseForm";
import { TaskTable } from "../../features/task-management/TaskTable";
import { api } from "../../services/api";

export function ProjectPage() {
  const { id = "" } = useParams();
  const project = useQuery({ queryKey: ["projects", id], queryFn: () => api.project(id) });
  const commands = useQuery({ queryKey: ["projects", id, "commands"], queryFn: () => api.projectCommands(id) });
  const tasks = useQuery({ queryKey: ["tasks", { project: id }], queryFn: () => api.tasks(id) });
  const p = project.data;

  if (project.error) return <ErrorBox error={project.error} />;
  if (!p) return <p className="muted">Загрузка…</p>;

  return (
    <>
      <h1>{p.name}</h1>
      <dl className="facts">
        <dt>Путь</dt>
        <dd className="mono">{p.local_path}</dd>
        {p.repository_url && (
          <>
            <dt>Репозиторий</dt>
            <dd className="mono">{p.repository_url}</dd>
          </>
        )}
        <dt>Лицензия</dt>
        <dd>
          {p.license ?? (
            <span className="warn">
              не определена — без лицензии код защищён авторским правом: изучайте локально, не распространяйте
            </span>
          )}
        </dd>
      </dl>

      <div className="two-col">
        <Card title="Языки">
          {p.languages.length === 0 ? (
            <Empty>Не обнаружены.</Empty>
          ) : (
            <table>
              <tbody>
                {p.languages.map((l) => (
                  <tr key={l.language}>
                    <td>{l.language}</td>
                    <td className="num">{l.files} файлов</td>
                    <td className="muted">{l.build_systems.join(", ")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Card>
        <Card title="Команды проверки">
          <ErrorBox error={commands.error} />
          {commands.data?.length === 0 && <Empty>Адаптеры не нашли команд.</Empty>}
          <ul className="list compact">
            {commands.data?.map((c, i) => (
              <li key={i}>
                <span className={`tag tag-${c.kind}`}>{c.kind}</span>
                <code>{[c.program, ...c.args].join(" ")}</code>
                {c.workdir && <span className="muted">в {c.workdir}</span>}
              </li>
            ))}
          </ul>
          <p className="hint">Запуск в изолированной среде появится на этапе 2 (Sandbox Manager).</p>
        </Card>
      </div>

      <Card title="Задачи">
        <ErrorBox error={tasks.error} />
        {tasks.data && <TaskTable tasks={tasks.data} />}
      </Card>
      <div className="two-col">
        <Card title="Новая задача">
          <CreateTaskForm projectId={p.id} />
        </Card>
        <Card title="Импорт упражнения">
          <ImportExerciseForm />
        </Card>
      </div>
    </>
  );
}
