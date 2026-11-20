import { clearAnswerDraft, useAnswerDraft } from "./answerDrafts";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { Icon } from "../app/icons";
import { ThemeToggle } from "../app/theme";
import { ContextPanel } from "../context-panels/ContextPanel";
import { useT } from "../i18n";
import { api } from "../services/api";
import { LEVEL_CODES, type Level, type Message } from "../types/api";
import { Badge, ErrorBox, levelLabel, levelName, shortId } from "../ui";
import { Composer } from "./Composer";
import { MessageList } from "./MessageList";

/** Bullet or numbered list items of a mentor answer → criteria texts. */
export function parseCriteria(markdown: string): string[] {
  return markdown
    .split("\n")
    .map((l) => l.match(/^\s*(?:[-*•]|\d+[.)])\s+(.*)$/)?.[1]?.trim())
    .filter((x): x is string => !!x)
    .map((x) => x.replace(/^\*\*(.*)\*\*$/, "$1"));
}

export function ConversationPage() {
  const { id = "" } = useParams();
  const { t } = useT();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const view = useQuery({ queryKey: ["conversation", id], queryFn: () => api.conversation(id) });
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions });
  const [pending, setPending] = useState<string | null>(null);
  const bottom = useRef<HTMLDivElement>(null);

  const refresh = () => qc.invalidateQueries({ queryKey: ["conversation", id] });
  const send = useMutation({
    mutationFn: ({ text, excerpt }: { text: string; excerpt?: string }) => api.postMessage(id, text, excerpt),
    onMutate: ({ text }) => setPending(text),
    // The draft stays until the stored answer is loaded.
    onSettled: async () => {
      await refresh();
      setPending(null);
      clearAnswerDraft(id);
    },
  });
  const settle = async () => {
    await refresh();
    clearAnswerDraft(id);
  };
  const accept = useMutation({ mutationFn: api.acceptEscalation, onSettled: settle });
  const reject = useMutation({ mutationFn: api.rejectEscalation, onSettled: settle });
  const raise = useMutation({ mutationFn: () => api.raiseLevel(id), onSettled: settle });
  const draft = useAnswerDraft(id);
  const saveCriteria = useMutation({
    mutationFn: (texts: string[]) => {
      const task = view.data!.task!;
      const existing = task.acceptance_criteria;
      const merged = [...existing, ...texts.filter((x) => !existing.some((e) => e.text === x)).map((text) => ({ text, done: false }))];
      return api.setCriteria(task.id, merged);
    },
    onSuccess: () => {
      refresh();
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });
  const busy = send.isPending || accept.isPending || reject.isPending || raise.isPending;

  const count = view.data?.messages.length ?? 0;
  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [count, pending, draft?.text, draft?.withheld]);

  if (view.error) return <ErrorBox error={view.error} />;
  const v = view.data;
  if (!v) return <p className="muted pad">{t("common.loading")}</p>;
  const { engagement, task } = v;
  const session =
    sessions.data?.filter((s) => s.conversation_id === id).sort((a, b) => Number(b.status === "active") - Number(a.status === "active"))[0] ?? null;
  const next = engagement.level < engagement.max_level ? ((engagement.level + 1) as Level) : null;

  return (
    <div className="workspace">
      <header className="chat-head">
        <button className="icon-btn" onClick={() => navigate(-1)} aria-label={t("common.back")}>
          <Icon.back />
        </button>
        <h1 title={v.conversation.title}>{v.conversation.title}</h1>
        {task && <span className="issue-id">#EST-{shortId(task.id)}</span>}
        {session && <Badge value={session.status} />}
        <div className="head-actions">
          <span className="level-pill" title={t("chat.levelHint", { max: LEVEL_CODES[engagement.max_level] })}>
            {levelLabel(t, engagement.level)}
          </span>
          {next !== null && (
            <button className="raise" onClick={() => raise.mutate()} disabled={busy} title={t("chat.raiseHint", { level: LEVEL_CODES[next], name: levelName(t, next) })}>
              {t("chat.raise", { level: LEVEL_CODES[next] })} <Icon.up />
            </button>
          )}
          <ThemeToggle />
        </div>
      </header>
      <div className="workspace-body">
        <section className="chat" aria-label={t("chat.aria")}>
          <div className="chat-scroll">
            {v.messages.length === 0 && !pending && (
              <div className="chat-empty">
                <h2>{t("chat.emptyTitle")}</h2>
                <p className="muted">{t("chat.emptyText")}</p>
              </div>
            )}
            <MessageList
              messages={v.messages}
              pending={pending && !isStored(v.messages, pending) ? pending : null}
              draft={busy ? draft : null}
              busy={busy}
              onAccept={(e) => accept.mutate(e)}
              onReject={(e) => reject.mutate(e)}
              onSaveCriteria={task ? (md) => saveCriteria.mutate(parseCriteria(md)) : undefined}
            />
            <div ref={bottom} />
          </div>
          <ErrorBox error={send.error ?? accept.error ?? reject.error ?? raise.error ?? saveCriteria.error} />
          <Composer disabled={busy} onSend={(text, excerpt) => send.mutate({ text, excerpt })} />
        </section>
        <ContextPanel task={task} session={session} onAsk={(text) => send.mutate({ text })} />
      </div>
    </div>
  );
}

/** The sent message is stored before the answer is written; show it once. */
function isStored(messages: Message[], pending: string) {
  const last = [...messages].reverse().find((m) => m.role === "user");
  return last?.content === pending.trim();
}
