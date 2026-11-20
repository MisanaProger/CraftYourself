import { useState } from "react";
import { useT } from "../i18n";
import type { LearningSession, Task } from "../types/api";
import { GitPanel } from "./GitPanel";
import { IssuePanel } from "./IssuePanel";
import { TestsPanel } from "./TestsPanel";

type Tab = "issue" | "git" | "tests";

export function ContextPanel({ task, session, onAsk }: { task: Task | null; session: LearningSession | null; onAsk: (text: string) => void }) {
  const { t } = useT();
  const [tab, setTab] = useState<Tab>("issue");
  const tabs: { id: Tab; label: string }[] = [
    { id: "issue", label: t("panel.issue") },
    { id: "git", label: "Git" },
    { id: "tests", label: t("panel.tests") },
  ];
  return (
    <aside className="context" aria-label={t("panel.aria")}>
      <div className="tabs" role="tablist">
        {tabs.map((x) => (
          <button key={x.id} role="tab" aria-selected={tab === x.id} className={tab === x.id ? "tab active" : "tab"} onClick={() => setTab(x.id)}>
            {x.label}
          </button>
        ))}
      </div>
      <div className="context-body">
        {tab === "issue" && <IssuePanel task={task} session={session} onAsk={onAsk} />}
        {tab === "git" && <GitPanel session={session} />}
        {tab === "tests" && <TestsPanel task={task} session={session} />}
      </div>
    </aside>
  );
}
