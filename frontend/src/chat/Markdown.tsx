import { Fragment, type ReactNode } from "react";

/**
 * Minimal, safe Markdown renderer for mentor messages: fenced code blocks,
 * paragraphs, lists, inline code, bold and italics. Everything is rendered
 * as React text nodes — no HTML from the model ever reaches the DOM.
 */
export function Markdown({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  const lines = text.split("\n");
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const fence = line.match(/^\s*```(\S*)/);
    if (fence) {
      const code: string[] = [];
      i++;
      while (i < lines.length && !/^\s*```/.test(lines[i])) code.push(lines[i++]);
      i++;
      blocks.push(<CodeBlock key={blocks.length} lang={fence[1]} code={code.join("\n")} />);
      continue;
    }
    if (/^\s*([-*]|\d+\.)\s+/.test(line)) {
      const items: string[] = [];
      const ordered = /^\s*\d+\./.test(line);
      while (i < lines.length && /^\s*([-*]|\d+\.)\s+/.test(lines[i])) items.push(lines[i++].replace(/^\s*([-*]|\d+\.)\s+/, ""));
      const List = ordered ? "ol" : "ul";
      blocks.push(
        <List key={blocks.length}>
          {items.map((t, k) => (
            <li key={k}>{inline(t)}</li>
          ))}
        </List>,
      );
      continue;
    }
    if (!line.trim()) {
      i++;
      continue;
    }
    const para: string[] = [];
    while (i < lines.length && lines[i].trim() && !/^\s*```/.test(lines[i]) && !/^\s*([-*]|\d+\.)\s+/.test(lines[i])) para.push(lines[i++]);
    blocks.push(<p key={blocks.length}>{para.map((p, k) => <Fragment key={k}>{k > 0 && <br />}{inline(p)}</Fragment>)}</p>);
  }
  return <div className="md">{blocks}</div>;
}

function inline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /(`[^`]+`|\*\*[^*]+\*\*|_[^_]+_)/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index! > last) out.push(text.slice(last, m.index));
    const t = m[0];
    if (t.startsWith("`")) out.push(<code key={out.length}>{t.slice(1, -1)}</code>);
    else if (t.startsWith("**")) out.push(<strong key={out.length}>{t.slice(2, -2)}</strong>);
    else out.push(<em key={out.length}>{t.slice(1, -1)}</em>);
    last = m.index! + t.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

function CodeBlock({ code, lang }: { code: string; lang: string }) {
  const lines = code.split("\n");
  return (
    <figure className="code-block">
      <figcaption>
        <span>{lang || "код"}</span>
        <button className="icon-btn small" onClick={() => navigator.clipboard?.writeText(code)} aria-label="Копировать код" title="Копировать">
          ⧉
        </button>
      </figcaption>
      <pre>
        {lines.map((l, k) => (
          <div key={k}>
            <span className="ln" aria-hidden>
              {k + 1}
            </span>
            {l || " "}
          </div>
        ))}
      </pre>
    </figure>
  );
}
