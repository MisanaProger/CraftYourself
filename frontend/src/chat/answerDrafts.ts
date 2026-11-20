import { useSyncExternalStore } from "react";
import type { DomainEvent } from "../types/api";

/**
 * Mentor answers while the model writes them (`ai.answer.*` events).
 * The server sends only text outside code blocks; a block is announced as
 * withheld and appears in the stored answer, which replaces the draft.
 */
export interface AnswerDraft {
  inReplyTo: string;
  text: string;
  /** Code blocks shown only after the policy check. */
  withheld: number;
  done: boolean;
}

const drafts = new Map<string, AnswerDraft>();
const listeners = new Set<() => void>();
const notify = () => listeners.forEach((l) => l());

/** Handles a live event; returns true if it was part of an answer stream. */
export function handleAnswerEvent(e: DomainEvent): boolean {
  if (!e.kind.startsWith("ai.answer.") || !e.conversation_id) return false;
  const id = e.conversation_id;
  const inReplyTo = String(e.payload.in_reply_to ?? "");
  const current = drafts.get(id);
  switch (e.kind) {
    case "ai.answer.started":
      drafts.set(id, { inReplyTo, text: "", withheld: 0, done: false });
      break;
    case "ai.answer.delta":
      if (!current || current.inReplyTo !== inReplyTo) return true;
      drafts.set(id, {
        ...current,
        text: current.text + (typeof e.payload.text === "string" ? e.payload.text : ""),
        withheld: current.withheld + (e.payload.withheld ? 1 : 0),
      });
      break;
    case "ai.answer.finished":
      if (current?.inReplyTo === inReplyTo) drafts.set(id, { ...current, done: true });
      break;
    case "ai.answer.failed":
      drafts.delete(id);
      break;
    default:
      return false;
  }
  notify();
  return true;
}

export function clearAnswerDraft(conversationId: string) {
  if (drafts.delete(conversationId)) notify();
}

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};

export function useAnswerDraft(conversationId: string): AnswerDraft | null {
  return useSyncExternalStore(subscribe, () => drafts.get(conversationId) ?? null);
}
