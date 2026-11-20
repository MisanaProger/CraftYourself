import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate } from "react-router";
import { Card, Empty, ErrorBox } from "../../components/ui";
import { api } from "../../services/api";

export function ProjectsPage() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const [source, setSource] = useState("");
  const add = useMutation({
    mutationFn: api.addProject,
    onSuccess: (p) => {
      qc.invalidateQueries({ queryKey: ["projects"] });
      navigate(`/projects/${p.id}`);
    },
  });

  return (
    <>
      <h1>Проекты</h1>
      <Card title="Подключить репозиторий">
        <form
          className="row"
          onSubmit={(e) => {
            e.preventDefault();
            if (source.trim()) add.mutate(source.trim());
          }}
        >
          <input
            value={source}
            onChange={(e) => setSource(e.target.value)}
            placeholder="/home/me/src/project или https://github.com/owner/repo"
            aria-label="Путь или URL репозитория"
          />
          <button disabled={add.isPending || !source.trim()}>{add.isPending ? "Подключение…" : "Подключить"}</button>
        </form>
        <p className="hint">Локальный репозиторий используется на месте; удалённый клонируется в кэш EST.</p>
        <ErrorBox error={add.error} />
      </Card>

      <ErrorBox error={projects.error} />
      {projects.data?.length === 0 && <Empty>Пока нет подключённых проектов.</Empty>}
      <div className="grid">
        {projects.data?.map((p) => (
          <Link key={p.id} to={`/projects/${p.id}`} className="card card-link">
            <h2>{p.name}</h2>
            <p className="muted mono small">{p.local_path}</p>
            <p className="tags">
              {p.languages.map((l) => (
                <span key={l.language} className="tag">
                  {l.language}
                </span>
              ))}
              {p.license ? <span className="tag tag-quiet">{p.license}</span> : <span className="tag tag-warn">лицензия?</span>}
            </p>
          </Link>
        ))}
      </div>
    </>
  );
}
