import type {
  AdiOverview,
  CommandSpec,
  Conversation,
  ConversationView,
  LearningSession,
  Message,
  NewTask,
  PrivacySettings,
  Project,
  SessionOverview,
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

  sessions: () => json<LearningSession[]>("GET", "/sessions"),
  session: (id: string) => json<SessionOverview>("GET", `/sessions/${id}`),
  diff: async (id: string, against: "worktree" | "staged" | "base") => (await send("GET", `/sessions/${id}/diff?against=${against}`)).text(),
  finish: (id: string, outcome: "completed" | "abandoned") => json<LearningSession>("POST", `/sessions/${id}/finish`, { outcome }),
  openWorkspace: (id: string) => json<{ command: string }>("POST", `/sessions/${id}/workspace/open`, {}),

  conversations: () => json<Conversation[]>("GET", "/conversations"),
  createConversation: (title?: string) => json<Conversation>("POST", "/conversations", { title }),
  conversation: (id: string) => json<ConversationView>("GET", `/conversations/${id}`),
  deleteConversation: (id: string) => send("DELETE", `/conversations/${id}`),
  postMessage: (id: string, content: string, excerpt?: string) =>
    json<Message[]>("POST", `/conversations/${id}/messages`, { content, excerpt: excerpt || undefined }),
  raiseLevel: (id: string) => json<Message[]>("POST", `/conversations/${id}/engagement/raise`, {}),
  acceptEscalation: (id: string) => json<Message[]>("POST", `/engagement/escalations/${id}/accept`, {}),
  rejectEscalation: (id: string) => json<Message[]>("POST", `/engagement/escalations/${id}/reject`, {}),

  adi: () => json<AdiOverview>("GET", "/analytics/adi"),
  skillMatrix: () => json<SkillMatrix>("GET", "/analytics/disciplines"),
  privacy: () => json<PrivacySettings>("GET", "/settings/privacy"),
  exportData: async () => (await send("POST", "/data/export", {})).blob(),
};
