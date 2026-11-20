import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../services/api";
import type { LearningSession } from "../types/api";
import { Empty, ErrorBox, shortHash } from "../ui";

type Target = "base" | "worktree" | "staged";

function lineClass(line: string) {
  if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("diff ") || line.startsWith("index ")) return "d-head";
  if (line.startsWith("@@")) return "d-hunk";
  if (line.startsWith("+")) return "d-add";
  if (line.startsWith("-")) return "d-del";
  return "";
}

export function GitPanel({ session }: { session: LearningSession | null }) {
  const [target, setTarget] = useState<Target>("base");
  const o = useQuery({ queryKey: ["session", session?.id], queryFn: () => api.session(session!.id), enabled: !!session, refetchInterval: 5000 });
  const version = o.data ? `${o.data.commits[0]?.hash}:${o.data.changes.map((c) => c.code + c.path).join()}` : "";
  const diff = useQuery({
    queryKey: ["session", session?.id, "diff", target, version],
    queryFn: () => api.diff(session!.id, target),
    enabled: !!session,
  });
  if (!session) return <Empty>Начните Issue, чтобы получить рабочий каталог.</Empty>;
  return (
    <div className="git">
      <ErrorBox error={o.error ?? diff.error} />
      <h3>Свои коммиты</h3>
      {o.data?.commits.length ? (
        <ul className="plain">
          {o.data.commits.map((c) => (
            <li key={c.hash}>
              <code>{shortHash(c.hash)}</code> {c.subject}
            </li>
          ))}
        </ul>
      ) : (
        <Empty>Пока нет.</Empty>
      )}
      <h3>Незафиксировано</h3>
      {o.data?.changes.length ? (
        <ul className="plain">
          {o.data.changes.map((c) => (
            <li key={c.path}>
              <code className="status">{c.code}</code> <span className="mono">{c.path}</span>
            </li>
          ))}
        </ul>
      ) : (
        <Empty>Рабочий каталог чист.</Empty>
      )}
      <div className="seg" role="tablist" aria-label="Что сравнивать">
        {(
          [
            ["base", "С начала задачи"],
            ["worktree", "Рабочий каталог"],
            ["staged", "Индекс"],
          ] as const
        ).map(([id, label]) => (
          <button key={id} role="tab" aria-selected={target === id} className={target === id ? "on" : ""} onClick={() => setTarget(id)}>
            {label}
          </button>
        ))}
      </div>
      {diff.data === "" && <Empty>Изменений нет.</Empty>}
      {diff.data && (
        <pre className="diff">
          {diff.data.split("\n").map((l, i) => (
            <div key={i} className={lineClass(l)}>
              {l || " "}
            </div>
          ))}
        </pre>
      )}
    </div>
  );
}
