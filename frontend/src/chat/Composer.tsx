import { useState } from "react";
import { Icon } from "../app/icons";
import { useT } from "../i18n";

export function Composer({ disabled, onSend }: { disabled: boolean; onSend: (text: string, excerpt?: string) => void }) {
  const { t } = useT();
  const [text, setText] = useState("");
  const [excerpt, setExcerpt] = useState("");
  const [showExcerpt, setShowExcerpt] = useState(false);
  const send = () => {
    if (!text.trim() || disabled) return;
    onSend(text.trim(), showExcerpt ? excerpt : undefined);
    setText("");
    setExcerpt("");
    setShowExcerpt(false);
  };
  return (
    <form
      className="composer"
      onSubmit={(e) => {
        e.preventDefault();
        send();
      }}
    >
      {showExcerpt && (
        <textarea className="excerpt" rows={3} value={excerpt} onChange={(e) => setExcerpt(e.target.value)} placeholder={t("composer.excerpt")} aria-label={t("composer.excerptAria")} />
      )}
      <div className="composer-row">
        <button
          type="button"
          className={`icon-btn ${showExcerpt ? "on" : ""}`}
          onClick={() => setShowExcerpt((v) => !v)}
          aria-label={t("composer.attach")}
          title={t("composer.attach")}
        >
          <Icon.clip />
        </button>
        <textarea
          rows={1}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
          placeholder={t("composer.placeholder")}
          aria-label={t("composer.aria")}
          disabled={disabled}
        />
        <button type="submit" className="send" disabled={disabled || !text.trim()} aria-label={t("composer.send")}>
          <Icon.send />
        </button>
      </div>
    </form>
  );
}
