import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { en, type Key } from "./en";
import { ru } from "./ru";
import { uz } from "./uz";

export type Lang = "ru" | "en" | "uz";
export const LANGS: { id: Lang; label: string }[] = [
  { id: "ru", label: "RU" },
  { id: "en", label: "EN" },
  { id: "uz", label: "UZ" },
];
const DICTS: Record<Lang, Record<Key, string>> = { ru, en, uz };
const STORAGE = "craft-lang";

function initial(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE);
    if (saved === "ru" || saved === "en" || saved === "uz") return saved;
  } catch {
    /* storage unavailable */
  }
  const nav = (navigator.language || "en").slice(0, 2).toLowerCase();
  return nav === "ru" || nav === "uz" ? nav : "en";
}

export type T = (key: Key, vars?: Record<string, string | number>) => string;

interface Ctx {
  lang: Lang;
  setLang: (l: Lang) => void;
  t: T;
}

const LocaleContext = createContext<Ctx | null>(null);

export function LocaleProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(initial);
  useEffect(() => {
    document.documentElement.lang = lang;
    try {
      localStorage.setItem(STORAGE, lang);
    } catch {
      /* storage unavailable */
    }
  }, [lang]);
  const t = useCallback<T>(
    (key, vars) => {
      let s = DICTS[lang][key] ?? en[key] ?? key;
      if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
      return s;
    },
    [lang],
  );
  const value = useMemo(() => ({ lang, setLang: setLangState, t }), [lang, t]);
  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export function useT() {
  const ctx = useContext(LocaleContext);
  if (!ctx) throw new Error("useT outside LocaleProvider");
  return ctx;
}

export function LangSwitch() {
  const { lang, setLang, t } = useT();
  return (
    <div className="lang-switch" role="group" aria-label={t("common.language")}>
      {LANGS.map((l) => (
        <button key={l.id} className={l.id === lang ? "on" : ""} aria-pressed={l.id === lang} onClick={() => setLang(l.id)}>
          {l.label}
        </button>
      ))}
    </div>
  );
}

export type { Key };
