import { useEffect, useRef, useState } from "react";

export interface FrameProgress {
  score: number;
  completed: boolean;
}

interface Props {
  src: string;
  title: string;
  initialHeight?: number;
  className?: string;
  onProgress?: (p: FrameProgress) => void;
}

/**
 * An untrusted document (a learning artifact or an HTML page written by a
 * model) in a sandboxed iframe: `allow-scripts` without `allow-same-origin`
 * gives it an opaque origin, so it cannot reach this page, its storage or
 * the API; the server adds a CSP that blocks the network. It may only post
 * `est:resize` and `est:progress` messages (`version: 1`), validated here.
 */
export function IsolatedFrame({ src, title, initialHeight = 420, className = "artifact", onProgress }: Props) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(initialHeight);
  const progress = useRef(onProgress);
  useEffect(() => {
    progress.current = onProgress;
  }, [onProgress]);
  useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      if (!frame.current || e.source !== frame.current.contentWindow || e.origin !== "null") return;
      const d = e.data as { type?: unknown; version?: unknown; score?: unknown; completed?: unknown; height?: unknown } | null;
      if (!d || typeof d !== "object" || d.version !== 1) return;
      if (d.type === "est:resize") {
        if (typeof d.height === "number" && Number.isFinite(d.height)) setHeight(Math.max(160, Math.min(3000, Math.round(d.height))));
        return;
      }
      if (d.type !== "est:progress" || !progress.current) return;
      if (typeof d.score !== "number" || !Number.isFinite(d.score) || typeof d.completed !== "boolean") return;
      progress.current({ score: Math.max(0, Math.min(100, Math.round(d.score))), completed: d.completed });
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);
  return <iframe ref={frame} className={className} style={{ height }} title={title} src={src} sandbox="allow-scripts" referrerPolicy="no-referrer" />;
}
