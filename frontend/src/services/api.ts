import type {
  CommandSpec,
  HelpLevel,
  LearningSession,
  MentorAnswer,
  NewTask,
  Project,
  SessionOverview,
  StartedSession,
  StoredEvent,
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

async function request<T>(method: string, path: string, body?: unknown, text = false): Promise<T> {
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
  return (text ? res.text() : res.json()) as Promise<T>;
}

export const api = {
  projects: () => request<Project[]>("GET", "/projects"),
  project: (id: string) => request<Project>("GET", `/projects/${id}`),
  projectCommands: (id: string) => request<CommandSpec[]>("GET", `/projects/${id}/commands`),
  addProject: (source: string) => request<Project>("POST", "/projects", { source }),

  tasks: (projectId?: string) =>
    request<Task[]>("GET", projectId ? `/tasks?project_id=${encodeURIComponent(projectId)}` : "/tasks"),
  task: (id: string) => request<Task>("GET", `/tasks/${id}`),
  createTask: (task: NewTask) => request<Task>("POST", "/tasks", task),
  importExercise: (manifest: string) => request<Task>("POST", "/exercises/import", { manifest }),
  startTask: (id: string) => request<StartedSession>("POST", `/tasks/${id}/start`, {}),

  sessions: () => request<LearningSession[]>("GET", "/sessions"),
  session: (id: string) => request<SessionOverview>("GET", `/sessions/${id}`),
  diff: (id: string, against: "worktree" | "staged" | "base") =>
    request<string>("GET", `/sessions/${id}/diff?against=${against}`, undefined, true),
  events: (id: string) => request<StoredEvent[]>("GET", `/sessions/${id}/events`),
  finish: (id: string, outcome: "completed" | "abandoned") =>
    request<LearningSession>("POST", `/sessions/${id}/finish`, { outcome }),
  openWorkspace: (id: string) => request<{ command: string }>("POST", `/sessions/${id}/workspace/open`, {}),
  ask: (id: string, question: string, level: HelpLevel, excerpt?: string) =>
    request<MentorAnswer>("POST", `/sessions/${id}/mentor/ask`, { question, level, excerpt: excerpt || undefined }),
  mentorHistory: (id: string) => request<StoredEvent[]>("GET", `/sessions/${id}/mentor/history`),
};
