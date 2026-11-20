import { useState } from "react";
import type { LearningSession, Task } from "../types/api";
import { GitPanel } from "./GitPanel";
import { IssuePanel } from "./IssuePanel";
import { TestsPanel } from "./TestsPanel";

type Tab = "issue" | "git" | "tests";

export function ContextPanel({ task, session }: { task: Task | null; session: LearningSession | null }) {
  const [tab, setTab] = useState<Tab>("issue");
  const tabs: { id: Tab; label: string }[] = [
    { id: "issue", label: "Issue" },
    { id: "git", label: "Git" },
    { id: "tests", label: "Tests" },
  ];
  return (
    <aside className="context" aria-label="Контекст задачи">
      <div className="tabs" role="tablist">
        {tabs.map((t) => (
          <button key={t.id} role="tab" aria-selected={tab === t.id} className={tab === t.id ? "tab active" : "tab"} onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>
      <div className="context-body">
        {tab === "issue" && <IssuePanel task={task} session={session} />}
        {tab === "git" && <GitPanel session={session} />}
        {tab === "tests" && <TestsPanel task={task} />}
      </div>
    </aside>
  );
}
