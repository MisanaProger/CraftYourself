import { useState } from "react";
import { Icon } from "../app/icons";

export function Composer({ disabled, onSend }: { disabled: boolean; onSend: (text: string, excerpt?: string) => void }) {
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
        <textarea
          className="excerpt"
          rows={3}
          value={excerpt}
          onChange={(e) => setExcerpt(e.target.value)}
          placeholder="Фрагмент документации с непонятным термином (до 1500 символов)"
          aria-label="Фрагмент документации"
        />
      )}
      <div className="composer-row">
        <button
          type="button"
          className={`icon-btn ${showExcerpt ? "on" : ""}`}
          onClick={() => setShowExcerpt((v) => !v)}
          aria-label="Приложить фрагмент документации"
          title="Приложить фрагмент документации"
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
          placeholder="Опиши, что ты уже проверил…"
          aria-label="Сообщение ментору"
          disabled={disabled}
        />
        <button type="submit" className="send" disabled={disabled || !text.trim()} aria-label="Отправить">
          <Icon.send />
        </button>
      </div>
    </form>
  );
}
