import type {
  AdiOverview,
  CommandSpec,
  Conversation,
  ConversationView,
  Criterion,
  GenerateProgram,
  ImportProblem,
  KnowledgeProfile,
  ProblemSummary,
  RecommendationSet,
  GeneratedProgram,
  LearningSession,
  LessonProgress,
  LessonView,
  LlmModelsQuery,
  LlmPreset,
  LlmProviderUpdate,
  LlmProviderView,
  LlmProviders,
  Message,
  NewLlmProvider,
  NewTask,
  PrivacySettings,
  ProgramTile,
  ProgramView,
  Project,
  QuizResult,
  RunRequest,
  SandboxInfo,
  SandboxRun,
  SessionOverview,
  SessionSummary,
  SkillMatrix,
  StartedSession,
  Task,
} from "../types/api";

const BASE = "/api/v1";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

async function send(method: string, path: string, body?: unknown): Promise<Response> {
  const res = await fetch(BASE + path, {
    method,
    headers: body === undefined ? undefined : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) {
    let message = res.statusText;
    try {
      message = (await res.json()).error ?? message;
    } catch {
      /* not JSON */
    }
    throw new ApiError(res.status, message);
  }
  return res;
}

const json = async <T>(method: string, path: string, body?: unknown) => (await send(method, path, body)).json() as Promise<T>;
const q = (lang: string) => `lang=${encodeURIComponent(lang)}`;

/** URLs of untrusted documents, loaded only into `<iframe sandbox="allow-scripts">`. */
export const pages = {
  lesson: (p: string, l: string, theme: string) =>
    `${BASE}/learning/programs/${encodeURIComponent(p)}/lessons/${encodeURIComponent(l)}/page?theme=${theme}`,
  message: (conversation: string, message: string, n: number, theme: string) =>
    `${BASE}/conversations/${encodeURIComponent(conversation)}/messages/${encodeURIComponent(message)}/pages/${n}?theme=${theme}`,
};

export const api = {
  projects: () => json<Project[]>("GET", "/projects"),
  project: (id: string) => json<Project>("GET", `/projects/${id}`),
  projectCommands: (id: string) => json<CommandSpec[]>("GET", `/projects/${id}/commands`),
  addProject: (source: string) => json<Project>("POST", "/projects", { source }),

  tasks: (projectId?: string) => json<Task[]>("GET", projectId ? `/tasks?project_id=${encodeURIComponent(projectId)}` : "/tasks"),
  task: (id: string) => json<Task>("GET", `/tasks/${id}`),
  createTask: (task: NewTask) => json<Task>("POST", "/tasks", task),
  importExercise: (manifest: string) => json<Task>("POST", "/exercises/import", { manifest }),
  startTask: (id: string) => json<StartedSession>("POST", `/tasks/${id}/start`, {}),
  completeTask: (id: string) => json<LearningSession>("POST", `/tasks/${id}/complete`, {}),
  setCriteria: (id: string, criteria: Criterion[]) => json<Task>("PUT", `/tasks/${id}/acceptance-criteria`, { criteria }),

  sessions: () => json<LearningSession[]>("GET", "/sessions"),
  session: (id: string) => json<SessionOverview>("GET", `/sessions/${id}`),
  diff: async (id: string, against: "worktree" | "staged" | "base") => (await send("GET", `/sessions/${id}/diff?against=${against}`)).text(),
  sessionSummary: (id: string) => json<SessionSummary>("GET", `/sessions/${id}/summary`),
  knowledgeProfile: (lang: string) => json<KnowledgeProfile>("GET", `/knowledge/profile?${q(lang)}`),
  openWorkspace: (id: string) => json<{ command: string }>("POST", `/sessions/${id}/workspace/open`, {}),

  sandbox: () => json<SandboxInfo>("GET", "/sandbox"),
  runs: (sessionId: string) => json<SandboxRun[]>("GET", `/sessions/${sessionId}/sandbox/runs`),
  startRun: (sessionId: string, req: RunRequest) => json<SandboxRun>("POST", `/sessions/${sessionId}/sandbox/runs`, req),

  conversations: () => json<Conversation[]>("GET", "/conversations"),
  createConversation: (title?: string) => json<Conversation>("POST", "/conversations", { title }),
  conversation: (id: string) => json<ConversationView>("GET", `/conversations/${id}`),
  deleteConversation: (id: string) => send("DELETE", `/conversations/${id}`),
  postMessage: (id: string, content: string, excerpt?: string) =>
    json<Message[]>("POST", `/conversations/${id}/messages`, { content, excerpt: excerpt || undefined }),
  raiseLevel: (id: string) => json<Message[]>("POST", `/conversations/${id}/engagement/raise`, {}),
  acceptEscalation: (id: string) => json<Message[]>("POST", `/engagement/escalations/${id}/accept`, {}),
  rejectEscalation: (id: string) => json<Message[]>("POST", `/engagement/escalations/${id}/reject`, {}),

  programs: (lang: string) => json<ProgramTile[]>("GET", `/learning/programs?${q(lang)}`),
  program: (id: string, lang: string) => json<ProgramView>("GET", `/learning/programs/${id}?${q(lang)}`),
  lesson: (p: string, l: string, lang: string) => json<LessonView>("GET", `/learning/programs/${p}/lessons/${l}?${q(lang)}`),
  completeLesson: (p: string, l: string) => json<LessonProgress>("POST", `/learning/programs/${p}/lessons/${l}/complete`, {}),
  submitQuiz: (p: string, l: string, answers: Record<string, number>, lang: string) =>
    json<QuizResult>("POST", `/learning/programs/${p}/lessons/${l}/quiz?${q(lang)}`, { answers }),
  artifactProgress: (p: string, l: string, score: number, completed: boolean) =>
    json<LessonProgress>("POST", `/learning/programs/${p}/lessons/${l}/progress`, { score, completed }),
  startPractice: (p: string, l: string) => json<StartedSession>("POST", `/learning/programs/${p}/lessons/${l}/practice`, {}),
  problems: () => json<ProblemSummary[]>("GET", "/problems"),
  importProblem: (p: ImportProblem) => json<StartedSession>("POST", "/problems", p),
  recommendations: () => json<RecommendationSet | null>("GET", "/recommendations"),
  recommend: (lang: string) => json<RecommendationSet>("POST", "/recommendations", { lang }),
  learningActivity: (lang: string) => json<unknown>("GET", `/recommendations/activity?${q(lang)}`),
  generatedPrograms: () => json<GeneratedProgram[]>("GET", "/learning/generated"),
  generateProgram: (req: GenerateProgram) => json<GeneratedProgram>("POST", "/learning/generated", req),
  deleteGeneratedProgram: (id: string) => send("DELETE", `/learning/generated/${encodeURIComponent(id)}`),

  adi: () => json<AdiOverview>("GET", "/analytics/adi"),
  skillMatrix: (lang: string) => json<SkillMatrix>("GET", `/analytics/disciplines?${q(lang)}`),
  privacy: () => json<PrivacySettings>("GET", "/settings/privacy"),
  exportData: async () => (await send("POST", "/data/export", {})).blob(),

  llmPresets: () => json<LlmPreset[]>("GET", "/llm/presets"),
  llmProviders: () => json<LlmProviders>("GET", "/llm/providers"),
  addLlmProvider: (p: NewLlmProvider) => json<LlmProviderView>("POST", "/llm/providers", p),
  updateLlmProvider: (id: string, p: LlmProviderUpdate) => json<LlmProviderView>("PATCH", `/llm/providers/${encodeURIComponent(id)}`, p),
  deleteLlmProvider: (id: string) => send("DELETE", `/llm/providers/${encodeURIComponent(id)}`),
  setActiveLlm: (id: string | null) => json<LlmProviders>("POST", "/llm/active", { id }),
  llmModels: (q: LlmModelsQuery) => json<string[]>("POST", "/llm/models", q),
};
