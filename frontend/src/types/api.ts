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
  acceptance_criteria: Criterion[];
  mentor: { max_level: Level; denied_paths: string[] };
  docs: { title: string; url: string; topics: string[] }[];
  created_at: string;
}

export interface Criterion {
  text: string;
  done: boolean;
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
  | { type: "policy_notice"; text: string; code: string | null }
  | {
      type: "sandbox_run";
      run_id: string;
      kind: RunKind;
      command: string;
      status: RunStatus;
      summary: TestSummary | null;
      exit_code: number | null;
      duration_ms: number | null;
    };

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
  code: string;
  count: number;
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

export type LlmProtocol = "openai-compatible" | "openai" | "anthropic";
export type LlmAuth = "api_key" | "subscription" | "none";
export type KeySource = "stored" | "env" | "missing" | "not_needed";

export interface LlmPreset {
  id: string;
  name: string;
  vendor: string;
  protocol: LlmProtocol;
  auth: LlmAuth;
  base_url: string;
  models: string[];
  api_key_env: string | null;
  key_url: string | null;
  docs_url: string;
}

export interface LlmProviderView {
  id: string;
  preset: string;
  name: string;
  protocol: LlmProtocol;
  auth: LlmAuth;
  base_url: string;
  model: string;
  api_key_env: string | null;
  key: KeySource;
  remote: boolean;
  active: boolean;
  read_only: boolean;
}

export interface LlmProviders {
  active: string | null;
  providers: LlmProviderView[];
}

export interface NewLlmProvider {
  preset: string;
  name?: string;
  base_url?: string;
  model: string;
  protocol?: LlmProtocol;
  api_key?: string;
  api_key_env?: string;
  activate?: boolean;
}

export interface LlmProviderUpdate {
  name?: string;
  base_url?: string;
  model?: string;
  api_key?: string;
  api_key_env?: string;
  clear_key?: boolean;
}

export interface LlmModelsQuery {
  provider_id?: string;
  preset?: string;
  base_url?: string;
  protocol?: LlmProtocol;
  api_key?: string;
  api_key_env?: string;
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

export const LEVEL_CODES = ["L0", "L1", "L2", "L3", "L4"] as const;

export type RunKind = "test" | "build" | "check" | "setup" | "custom";
export type RunStatus = "running" | "passed" | "failed" | "timed_out" | "error";

export interface TestSummary {
  passed: number;
  failed: number;
  ignored: number;
}

export interface SandboxRun {
  id: string;
  session_id: string;
  task_id: string;
  kind: RunKind;
  command: string;
  workdir: string;
  image: string | null;
  network: boolean;
  status: RunStatus;
  exit_code: number | null;
  summary: TestSummary | null;
  duration_ms: number | null;
  stdout: string;
  stderr: string;
  error: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface RunRequest {
  kind?: RunKind;
  command?: string;
  allow_network?: boolean;
}

export interface SandboxInfo {
  runtime: string | null;
  available: boolean;
  reason: string | null;
  limits: Record<string, unknown>;
}

export type LessonKind = "reading" | "quiz" | "artifact" | "practice";
export type LessonStatus = "not_started" | "started" | "completed";

export interface ProgramTile {
  id: string;
  title: string;
  summary: string;
  status: "available" | "coming_soon";
  icon: string;
  level: string;
  minutes: number;
  disciplines: string[];
  lessons: number;
  completed: number;
}

export interface LessonSummary {
  id: string;
  kind: LessonKind;
  title: string;
  minutes: number;
  status: LessonStatus;
  score: number | null;
}

export interface ProgramView {
  id: string;
  title: string;
  summary: string;
  disciplines: string[];
  lessons: LessonSummary[];
}

export type LessonBody =
  | { kind: "reading"; markdown: string }
  | { kind: "quiz"; questions: { id: string; text: string; code: string | null; options: string[] }[]; pass_percent: number }
  | { kind: "artifact"; artifact_id: string; entrypoint: string; objectives: string[] }
  | { kind: "practice"; conversation_id: string | null; session_id: string | null; task_status: TaskStatus | null };

export interface LessonView extends LessonSummary {
  program_id: string;
  intro: string;
  body: LessonBody;
  previous: string | null;
  next: string | null;
}

export interface LessonProgress {
  program_id: string;
  lesson_id: string;
  status: LessonStatus;
  score: number | null;
  attempts: number;
}

export interface QuizResult {
  correct: number;
  total: number;
  score: number;
  passed: boolean;
  results: { id: string; chosen: number | null; correct: boolean; answer: number; explanation: string }[];
}
