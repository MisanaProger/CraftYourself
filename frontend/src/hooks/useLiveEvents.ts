import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { handleAnswerEvent } from "../chat/answerDrafts";
import type { DomainEvent } from "../types/api";

/** Subscribes to `/api/v1/events` and refreshes affected queries. */
export function useLiveEvents() {
  const qc = useQueryClient();
  useEffect(() => {
    let socket: WebSocket | null = null;
    let retry: number | undefined;
    let closed = false;

    const connect = () => {
      const proto = location.protocol === "https:" ? "wss" : "ws";
      socket = new WebSocket(`${proto}://${location.host}/api/v1/events`);
      socket.onmessage = (msg) => {
        const e = JSON.parse(msg.data) as DomainEvent;
        // A streamed answer updates its draft, not the queries.
        if (handleAnswerEvent(e)) return;
        if (e.kind.startsWith("project.")) qc.invalidateQueries({ queryKey: ["projects"] });
        if (e.kind.startsWith("task.") || e.kind.startsWith("session.")) {
          qc.invalidateQueries({ queryKey: ["tasks"] });
          qc.invalidateQueries({ queryKey: ["sessions"] });
        }
        if (e.kind.startsWith("adi.")) qc.invalidateQueries({ queryKey: ["adi"] });
        if (e.kind.startsWith("learning.program.")) qc.invalidateQueries({ queryKey: ["generated"] });
        if (e.kind.startsWith("problem.") || e.kind.startsWith("task.") || e.kind.startsWith("session.")) qc.invalidateQueries({ queryKey: ["problems"] });
        if (e.kind.startsWith("recommendations.")) qc.invalidateQueries({ queryKey: ["recommendations"] });
        if (e.conversation_id) {
          qc.invalidateQueries({ queryKey: ["conversation", e.conversation_id] });
          qc.invalidateQueries({ queryKey: ["conversations"] });
        }
        if (e.session_id) qc.invalidateQueries({ queryKey: ["session", e.session_id] });
      };
      socket.onclose = () => {
        if (!closed) retry = window.setTimeout(connect, 3000);
      };
    };
    connect();
    return () => {
      closed = true;
      window.clearTimeout(retry);
      socket?.close();
    };
  }, [qc]);
}
