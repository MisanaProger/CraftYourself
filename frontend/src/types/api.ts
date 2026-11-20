// Types mirror the JSON produced by the Rust API (`/api/v1`).

export type TaskType = "bug-fix" | "feature" | "refactoring" | "documentation" | "performance" | "testing";
export type Difficulty = "beginner" | "intermediate" | "advanced";
export type TaskStatus = "open" | "in-progress" | "done" | "archived";
export type SessionStatus = "active" | "completed" | "abandoned";
export type HelpLevel = 0 | 1 | 2 | 3 | 4;

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

export interface MentorSettings {
  max_help_level: HelpLevel;
  min_attempts_before_solution: number;
  denied_paths: string[];
}

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
  skills: string[];
  base_revision: string | null;
  test_command: string | null;
  mentor: MentorSettings;
  docs: { title: string; url: string; topics: string[] }[];
  created_at: string;
}

export interface NewTask {
  project_id: string;
  title: string;
  description: string;
  task_type: TaskType;
  difficulty: Difficulty;
  base_revision?: string;
}

export interface LearningSession {
  id: string;
  user_id: string;
  project_id: string;
  task_id: string;
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

export interface DocLink {
  kind: "curated" | "source" | "reference_search";
  title: string;
  target: string;
  symbol: string | null;
}

export interface MentorAnswer {
  kind: { type: "api_reference"; symbols: string[] } | { type: "conceptual" } | { type: "mentoring" };
  help: { requested: HelpLevel; granted: HelpLevel; limited_by: string | null };
  boundary:
    | { action: "answer" }
    | { action: "redirect"; symbols: string[] }
    | { action: "explain_excerpt"; max_quote_chars: number }
    | { action: "refuse"; reason: string };
  constraints: { code: string; max_code_lines: number | null; questions_only: boolean };
  links: DocLink[];
  forward_to_model: boolean;
  note: string;
}

export interface DomainEvent {
  kind: string;
  session_id?: string;
  project_id?: string;
  task_id?: string;
  payload: Record<string, unknown>;
  at: string;
}

export interface StoredEvent extends DomainEvent {
  id: number;
}

export const HELP_LEVELS: { level: HelpLevel; label: string }[] = [
  { level: 0, label: "Без подсказок" },
  { level: 1, label: "Направляющие вопросы" },
  { level: 2, label: "Концептуальные подсказки" },
  { level: 3, label: "Частичный пример" },
  { level: 4, label: "Подробный разбор" },
];
