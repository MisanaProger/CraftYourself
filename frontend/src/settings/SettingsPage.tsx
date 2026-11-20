import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../services/api";
import { Card, Empty, ErrorBox, PageTitle, formatDate } from "../ui";

export function SettingsPage() {
  const qc = useQueryClient();
  const privacy = useQuery({ queryKey: ["privacy"], queryFn: api.privacy });
  const conversations = useQuery({ queryKey: ["conversations"], queryFn: api.conversations });
  const exp = useMutation({
    mutationFn: api.exportData,
    onSuccess: (blob) => {
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `craft-yourself-export-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    },
  });
  const del = useMutation({
    mutationFn: (id: string) => api.deleteConversation(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["conversations"] });
      qc.invalidateQueries({ queryKey: ["adi"] });
    },
  });
  const llm = privacy.data?.llm;
  return (
    <div className="page">
      <PageTitle sub="Где хранятся данные, куда уходит контекст и как забрать или удалить историю.">Настройки и приватность</PageTitle>
      <div className="two-col">
        <Card title="Хранение и модель">
          <dl className="kv">
            <dt>Данные</dt>
            <dd>локально, в ~/.est</dd>
            <dt>Модель ИИ</dt>
            <dd>{llm ? `${llm.model} · ${llm.endpoint}` : "не подключена"}</dd>
            <dt>Передача контекста</dt>
            <dd>{llm ? (llm.remote ? <span className="warn">внешнему провайдеру</span> : "остаётся на устройстве") : "нет"}</dd>
          </dl>
          <p className="muted small">Провайдер задаётся в ~/.est/config/config.toml, раздел [llm]. Ключ API хранится в переменной окружения, не в файле.</p>
        </Card>
        <Card title="Экспорт">
          <p className="muted">Проекты, Issues, сессии, диалоги, события и оценки ADI — одним JSON-файлом.</p>
          <button className="primary" onClick={() => exp.mutate()} disabled={exp.isPending}>
            Экспортировать историю
          </button>
          <ErrorBox error={exp.error} />
        </Card>
      </div>
      <Card title="Диалоги">
        <p className="muted small">Удаление стирает сообщения, историю эскалаций, события диалога и ADI связанной Issue.</p>
        <ErrorBox error={del.error ?? conversations.error} />
        {conversations.data?.length === 0 && <Empty>Диалогов нет.</Empty>}
        <ul className="list">
          {conversations.data?.map((c) => (
            <li key={c.id}>
              <span>{c.title}</span>
              <span className="muted small">{formatDate(c.updated_at)}</span>
              <button
                className="danger small push"
                disabled={del.isPending}
                onClick={() => {
                  if (confirm(`Удалить диалог «${c.title}» и связанные данные? Это необратимо.`)) del.mutate(c.id);
                }}
              >
                Удалить
              </button>
            </li>
          ))}
        </ul>
      </Card>
    </div>
  );
}
