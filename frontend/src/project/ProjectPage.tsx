import { useQuery } from "@tanstack/react-query";
import { useParams } from "react-router";
import { CreateTaskForm } from "../issue/CreateTaskForm";
import { ImportExerciseForm } from "../issue/ImportExerciseForm";
import { TaskTable } from "../issue/TaskTable";
import { api } from "../services/api";
import { Card, Empty, ErrorBox, PageTitle } from "../ui";

export function ProjectPage() {
  const { id = "" } = useParams();
  const project = useQuery({ queryKey: ["projects", id], queryFn: () => api.project(id) });
  const commands = useQuery({ queryKey: ["projects", id, "commands"], queryFn: () => api.projectCommands(id) });
  const tasks = useQuery({ queryKey: ["tasks", { project: id }], queryFn: () => api.tasks(id) });
  const p = project.data;
  if (project.error) return <ErrorBox error={project.error} />;
  if (!p) return <p className="muted pad">Загрузка…</p>;
  return (
    <div className="page">
      <PageTitle sub={<span className="mono">{p.local_path}</span>}>{p.name}</PageTitle>
      {!p.license && (
        <p className="warn-box">Лицензия не определена: без лицензии код защищён авторским правом. Изучайте его локально и не распространяйте.</p>
      )}
      <div className="two-col">
        <Card title="Языки">
          {p.languages.length === 0 ? (
            <Empty>Не обнаружены.</Empty>
          ) : (
            <ul className="plain">
              {p.languages.map((l) => (
                <li key={l.language}>
                  <strong>{l.language}</strong> <span className="muted">· {l.files} файлов {l.build_systems.length > 0 && `· ${l.build_systems.join(", ")}`}</span>
                </li>
              ))}
            </ul>
          )}
          {p.license && <p className="muted small">Лицензия: {p.license}</p>}
        </Card>
        <Card title="Команды проверки">
          <ErrorBox error={commands.error} />
          {commands.data?.length === 0 && <Empty>Адаптеры не нашли команд.</Empty>}
          <ul className="plain">
            {commands.data?.map((c, i) => (
              <li key={i}>
                <span className={`tag tag-${c.kind}`}>{c.kind}</span> <code>{[c.program, ...c.args].join(" ")}</code>
              </li>
            ))}
          </ul>
        </Card>
      </div>
      <Card title="Issues проекта">
        <ErrorBox error={tasks.error} />
        {tasks.data && <TaskTable tasks={tasks.data} />}
      </Card>
      <div className="two-col">
        <Card title="Новая Issue">
          <CreateTaskForm projectId={p.id} />
        </Card>
        <Card title="Импорт упражнения">
          <ImportExerciseForm />
        </Card>
      </div>
    </div>
  );
}
