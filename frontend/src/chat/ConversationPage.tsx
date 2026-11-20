import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { Icon } from "../app/icons";
import { ThemeToggle } from "../app/theme";
import { ContextPanel } from "../context-panels/ContextPanel";
import { api } from "../services/api";
import { LEVELS } from "../types/api";
import { Badge, ErrorBox, levelLabel, shortId } from "../ui";
import { Composer } from "./Composer";
import { MessageList } from "./MessageList";

export function ConversationPage() {
  const { id = "" } = useParams();
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
    onSettled: () => {
      setPending(null);
      refresh();
    },
  });
  const accept = useMutation({ mutationFn: api.acceptEscalation, onSettled: refresh });
  const reject = useMutation({ mutationFn: api.rejectEscalation, onSettled: refresh });
  const raise = useMutation({ mutationFn: () => api.raiseLevel(id), onSettled: refresh });
  const busy = send.isPending || accept.isPending || reject.isPending || raise.isPending;

  const count = view.data?.messages.length ?? 0;
  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [count, pending]);

  if (view.error) return <ErrorBox error={view.error} />;
  const v = view.data;
  if (!v) return <p className="muted pad">Загрузка…</p>;
  const { engagement, task } = v;
  const session =
    sessions.data?.filter((s) => s.conversation_id === id).sort((a, b) => Number(b.status === "active") - Number(a.status === "active"))[0] ?? null;
  const next = engagement.level < engagement.max_level ? LEVELS[engagement.level + 1] : null;

  return (
    <div className="workspace">
      <header className="chat-head">
        <button className="icon-btn" onClick={() => navigate(-1)} aria-label="Назад">
          <Icon.back />
        </button>
        <h1 title={v.conversation.title}>{v.conversation.title}</h1>
        {task && <span className="issue-id">#EST-{shortId(task.id)}</span>}
        {session && <Badge value={session.status} />}
        <div className="head-actions">
          <span className="level-pill" title={`Уровень помощи в этой Issue. Максимум: L${engagement.max_level}`}>
            {levelLabel(engagement.level)}
          </span>
          {next && (
            <button
              className="raise"
              onClick={() => raise.mutate()}
              disabled={busy}
              title={`Повысить уровень помощи до ${next.code} (${next.name.toLowerCase()})`}
            >
              Запросить {next.code} <Icon.up />
            </button>
          )}
          <ThemeToggle />
        </div>
      </header>
      <div className="workspace-body">
        <section className="chat" aria-label="Диалог">
          <div className="chat-scroll">
            {v.messages.length === 0 && !pending && (
              <div className="chat-empty">
                <h2>Начните с того, что вы уже выяснили</h2>
                <p className="muted">
                  Новая Issue начинается с уровня L0: ментор задаёт вопросы, а не выдаёт решение. Если запрос потребует большего, Craft Yourself
                  предложит повысить уровень — на одну ступень и только с вашего согласия.
                </p>
              </div>
            )}
            <MessageList messages={v.messages} pending={pending} busy={busy} onAccept={(e) => accept.mutate(e)} onReject={(e) => reject.mutate(e)} />
            <div ref={bottom} />
          </div>
          <ErrorBox error={send.error ?? accept.error ?? reject.error ?? raise.error} />
          <Composer disabled={busy} onSend={(text, excerpt) => send.mutate({ text, excerpt })} />
        </section>
        <ContextPanel task={task} session={session} />
      </div>
    </div>
  );
}
