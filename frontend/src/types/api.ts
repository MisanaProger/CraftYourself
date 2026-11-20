// Types mirror the JSON produced by the Rust API (`/api/v1`).

export type TaskType = "bug-fix" | "feature" | "refactoring" | "documentation" | "performance" | "testing" | "regression";
export type Difficulty = "beginner" | "intermediate" | "advanced";
export type TaskStatus = "open" | "in-progress" | "done" | "archived";
export type SessionStatus = "active" | "completed" | "abandoned";
export type Level = 0 | 1 | 2 | 3 | 4;

export interface LanguageDetection {
  language: string;
  files: number;
  build_systems: string[];
}

export interface Project {
  id: string;
  name: string;
  repository_url: string | null;
  local_path: string;
  primary_language: string | null;
  languages: LanguageDetection[];
  license: string | null;
  created_at: string;
}

export interface CommandSpec {
  kind: "build" | "test" | "check";
  language: string;
  program: string;
  args: string[];
  workdir: string;
  description: string;
}

export type TaskSource =
  | { type: "manual" }
  | { type: "exercise"; id: string; version: string }
  | { type: "issue"; url: string };

export interface Task {
  id: string;
  project_id: string;
  title: string;
  description: string;
  task_type: TaskType;
  difficulty: Difficulty;
  source: TaskSource;
  status: TaskStatus;
  learning_goals: string[];
  disciplines: string[];
  base_revision: string | null;
  test_command: string | null;
  mentor: { max_level: Level; denied_paths: string[] };
  docs: { title: string; url: string; topics: string[] }[];
  created_at: string;
}

export interface NewTask {
  project_id: string;
  title: string;
  description: string;
  task_type: TaskType;
  difficulty: Difficulty;
  disciplines?: string[];
}

export interface LearningSession {
  id: string;
  user_id: string;
  project_id: string;
  task_id: string;
  conversation_id: string | null;
  workspace_path: string;
  branch: string;
  base_revision: string;
  status: SessionStatus;
  started_at: string;
  finished_at: string | null;
}

export interface StartedSession extends LearningSession {
  resumed: boolean;
}

export interface FileStatus {
  code: string;
  path: string;
}

export interface CommitSummary {
  hash: string;
  author: string;
  date: string;
  subject: string;
}

export interface SessionOverview {
  session: LearningSession;
  task: Task;
  project: Project;
  current_branch: string | null;
  changes: FileStatus[];
  commits: CommitSummary[];
}

export interface SourceLink {
  kind: string;
  title: string;
  target: string;
}

export type Card =
  | { type: "sources"; symbols: string[]; links: SourceLink[] }
  | { type: "escalation"; escalation_id: string; from: Level; to: Level; unlocks: string; status: "proposed" | "accepted" | "rejected" }
  | { type: "policy_notice"; text: string };

export interface Message {
  id: string;
  conversation_id: string;
  role: "user" | "assistant" | "system";
  content: string;
  cards: Card[];
  level: Level;
  meta: Record<string, unknown> | null;
  created_at: string;
}

export interface Conversation {
  id: string;
  project_id: string | null;
  task_id: string | null;
  title: string;
  status: "active" | "archived";
  created_at: string;
  updated_at: string;
}

export interface EngagementState {
  conversation_id: string;
  level: Level;
  max_level: Level;
  updated_at: string;
}

export interface Escalation {
  id: string;
  from: Level;
  to: Level;
  required: Level;
  status: "proposed" | "accepted" | "rejected";
}

export interface ConversationView {
  conversation: Conversation;
  engagement: EngagementState;
  pending_escalation: Escalation | null;
  messages: Message[];
  task: Task | null;
}

export interface AdiFactor {
  description: string;
  points: number;
}

export interface AdiHistoryItem {
  task_id: string;
  task_title: string | null;
  score: number;
  scoring_version: string;
  observed_features: Record<string, unknown>;
  factors: AdiFactor[];
  explanation: string;
  confidence: string;
  created_at: string;
}

export interface AdiOverview {
  aggregate: {
    value: number | null;
    issues: number;
    provisional: boolean;
    confidence: "low" | "medium" | "high";
    status: string | null;
    observed_since: string | null;
  };
  status_label: string | null;
  disclaimer: string;
  params: { half_life_days: number; half_life_issues: number; min_issues_for_status: number };
  history: AdiHistoryItem[];
}

export type AssessmentStatus = "not_assessed" | "insufficient_evidence" | "provisional" | "established" | "needs_reassessment";

export interface MatrixDiscipline {
  id: string;
  name: string;
  group: "fundamental" | "engineering";
  status: AssessmentStatus;
  score: number | null;
  practice_tasks: number;
  skills: { id: string; name: string; status: AssessmentStatus; score: number | null; evidence_count: number }[];
}

export interface SkillMatrix {
  catalog_version: string;
  disciplines: MatrixDiscipline[];
  note: string;
}

export interface PrivacySettings {
  storage: string;
  llm: { name: string; model: string; endpoint: string; remote: boolean } | null;
  history_analytics: boolean;
}

export interface DomainEvent {
  kind: string;
  session_id?: string;
  project_id?: string;
  task_id?: string;
  conversation_id?: string;
  payload: Record<string, unknown>;
  at: string;
}

export const LEVELS: { level: Level; code: string; name: string }[] = [
  { level: 0, code: "L0", name: "Сократический" },
  { level: 1, code: "L1", name: "Наводящий" },
  { level: 2, code: "L2", name: "Объясняющий" },
  { level: 3, code: "L3", name: "Демонстрационный" },
  { level: 4, code: "L4", name: "Активный" },
];
