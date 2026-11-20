import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Empty, ErrorBox } from "../../components/ui";
import { api } from "../../services/api";

type Target = "worktree" | "staged" | "base";
const TABS: { id: Target; label: string }[] = [
  { id: "worktree", label: "Не проиндексировано" },
  { id: "staged", label: "Проиндексировано" },
  { id: "base", label: "Вся работа с начала задачи" },
];

function lineClass(line: string) {
  if (line.startsWith("+++") || line.startsWith("---")) return "d-file";
  if (line.startsWith("@@")) return "d-hunk";
  if (line.startsWith("diff ")) return "d-head";
  if (line.startsWith("+")) return "d-add";
  if (line.startsWith("-")) return "d-del";
  return "";
}

export function DiffView({ sessionId, version }: { sessionId: string; version: string }) {
  const [target, setTarget] = useState<Target>("base");
  const diff = useQuery({
    queryKey: ["session", sessionId, "diff", target, version],
    queryFn: () => api.diff(sessionId, target),
  });
  return (
    <>
      <div className="tabs" role="tablist">
        {TABS.map((t) => (
          <button key={t.id} role="tab" aria-selected={target === t.id} className={target === t.id ? "tab active" : "tab"} onClick={() => setTarget(t.id)}>
            {t.label}
          </button>
        ))}
      </div>
      <ErrorBox error={diff.error} />
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
    </>
  );
}
