import { Fragment, useEffect, useRef, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { IsolatedFrame } from "../ui/IsolatedFrame";

/**
 * Minimal, safe Markdown renderer for mentor messages and lessons: headings,
 * fenced code blocks, tables, paragraphs, lists, inline code, bold, italics
 * and LaTeX formulas. Text is rendered as React text nodes — no HTML from the
 * model ever reaches this page's DOM. Formulas are built by KaTeX as DOM
 * nodes (`trust: false`: no links, classes, styles or raw HTML). An HTML page
 * from the model (a ```html block) is never inserted here: with `pageUrl` it
 * is loaded from the server into an isolated frame.
 */
const isTableRow = (l: string) => /^\s*\|.*\|\s*$/.test(l);
const cells = (l: string) => l.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
const BLOCK_START = /^\s*(```|#{1,4}\s|([-*]|\d+\.)\s+|\||\$\$|\\\[)/;

interface Props {
  text: string;
  /** URL of the n-th ```html block served as an isolated document. */
  pageUrl?: (n: number) => string;
}

export function Markdown({ text, pageUrl }: Props) {
  const blocks: ReactNode[] = [];
  const lines = text.split("\n");
  let pages = 0;
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const fence = line.match(/^\s*```(\S*)/);
    if (fence) {
      const code: string[] = [];
      i++;
      while (i < lines.length && !/^\s*```/.test(lines[i])) code.push(lines[i++]);
      i++;
      const lang = fence[1].toLowerCase();
      // Numbered like the server does: every ```html block, in order.
      const page = lang === "html" ? pages++ : -1;
      blocks.push(
        page >= 0 && pageUrl ? (
          <HtmlPage key={blocks.length} src={pageUrl(page)} code={code.join("\n")} />
        ) : (
          <CodeBlock key={blocks.length} lang={fence[1]} code={code.join("\n")} />
        ),
      );
      continue;
    }
    const math = line.trim().match(/^(\$\$|\\\[)(.*)$/);
    if (math) {
      const close = math[1] === "$$" ? "$$" : "\\]";
      const body: string[] = [];
      let rest = math[2];
      i++;
      while (!rest.trimEnd().endsWith(close) && i < lines.length) {
        body.push(rest);
        rest = lines[i++];
      }
      body.push(rest.trimEnd().endsWith(close) ? rest.trimEnd().slice(0, -close.length) : rest);
      blocks.push(<Tex key={blocks.length} tex={body.join("\n").trim()} display />);
      continue;
    }
    const heading = line.match(/^(#{1,4})\s+(.*)$/);
    if (heading) {
      const level = Math.min(heading[1].length + 1, 5);
      const H = `h${level}` as "h2" | "h3" | "h4" | "h5";
      blocks.push(<H key={blocks.length}>{inline(heading[2])}</H>);
      i++;
      continue;
    }
    if (isTableRow(line) && i + 1 < lines.length && /^\s*\|[\s:|-]+\|\s*$/.test(lines[i + 1])) {
      const head = cells(line);
      i += 2;
      const rows: string[][] = [];
      while (i < lines.length && isTableRow(lines[i])) rows.push(cells(lines[i++]));
      blocks.push(
        <div className="md-table" key={blocks.length}>
          <table>
            <thead>
              <tr>
                {head.map((h, k) => (
                  <th key={k}>{inline(h)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((r, k) => (
                <tr key={k}>
                  {r.map((c, j) => (
                    <td key={j}>{inline(c)}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
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
    while (i < lines.length && lines[i].trim() && (para.length === 0 || !BLOCK_START.test(lines[i]))) para.push(lines[i++]);
    blocks.push(<p key={blocks.length}>{para.map((p, k) => <Fragment key={k}>{k > 0 && <br />}{inline(p)}</Fragment>)}</p>);
  }
  return <div className="md">{blocks}</div>;
}

// Inline code, $$display$$, $inline$ (not "$5 and $10"), \(inline\), bold, italics.
const INLINE = /(`[^`]+`|\$\$[^$]+?\$\$|\$(?=[^\s$])[^$\n]*?[^\s$\\]\$(?!\d)|\$[^\s$]\$(?!\d)|\\\(.+?\\\)|\*\*[^*]+\*\*|_[^_]+_)/g;

function inline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  for (const m of text.matchAll(INLINE)) {
    if (m.index! > last) out.push(text.slice(last, m.index));
    const t = m[0];
    if (t.startsWith("`")) out.push(<code key={out.length}>{t.slice(1, -1)}</code>);
    else if (t.startsWith("$$")) out.push(<Tex key={out.length} tex={t.slice(2, -2)} display />);
    else if (t.startsWith("$")) out.push(<Tex key={out.length} tex={t.slice(1, -1)} />);
    else if (t.startsWith("\\(")) out.push(<Tex key={out.length} tex={t.slice(2, -2)} />);
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
        <span>{lang || "code"}</span>
        <button className="icon-btn small" onClick={() => navigator.clipboard?.writeText(code)} aria-label="Copy" title="Copy">
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

type Katex = typeof import("katex").default;
let katex: Promise<Katex> | null = null;

/** KaTeX and its fonts load on first use, not with the app. */
function loadKatex(): Promise<Katex> {
  katex ??= Promise.all([import("katex"), import("katex/dist/katex.min.css")]).then(([k]) => k.default);
  return katex;
}

/**
 * A LaTeX formula. KaTeX builds DOM nodes itself (no `innerHTML`); with
 * `trust: false` commands like \\href or \\htmlClass are rejected, and size
 * limits stop pathological input. Until it loads, the source is shown.
 */
function Tex({ tex, display = false }: { tex: string; display?: boolean }) {
  const target = useRef<HTMLSpanElement>(null);
  const [ready, setReady] = useState(false);
  useEffect(() => {
    let live = true;
    loadKatex()
      .then((k) => {
        if (!live || !target.current) return;
        k.render(tex, target.current, {
          displayMode: display,
          throwOnError: false,
          trust: false,
          strict: "ignore",
          maxSize: 40,
          maxExpand: 300,
          output: "htmlAndMathml",
        });
        setReady(true);
      })
      .catch(() => live && setReady(false));
    return () => {
      live = false;
    };
  }, [tex, display]);
  const Tag = display ? "div" : "span";
  return (
    <Tag className={display ? "math math-display" : "math"}>
      {!ready && <code className="math-src">{tex}</code>}
      <span ref={target} />
    </Tag>
  );
}

/** A ```html block from the model: preview in an isolated frame, or source. */
function HtmlPage({ src, code }: { src: string; code: string }) {
  const { t } = useT();
  const [source, setSource] = useState(false);
  return (
    <div className="html-page">
      <div className="html-page-bar" role="tablist">
        <button role="tab" aria-selected={!source} className="small" onClick={() => setSource(false)}>
          {t("md.preview")}
        </button>
        <button role="tab" aria-selected={source} className="small" onClick={() => setSource(true)}>
          {t("md.source")}
        </button>
      </div>
      {source ? (
        <CodeBlock lang="html" code={code} />
      ) : (
        <>
          <IsolatedFrame src={src} title="HTML" initialHeight={360} className="artifact page-frame" />
          <p className="muted small">{t("md.pageNote")}</p>
        </>
      )}
    </div>
  );
}
