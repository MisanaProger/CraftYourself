import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { LangSwitch, useT } from "../i18n";
import { api } from "../services/api";
import { Card, Empty, ErrorBox, PageTitle, useFormat } from "../ui";
import { AiProviders } from "./AiProviders";

export function SettingsPage() {
  const { t } = useT();
  const fmt = useFormat();
  const qc = useQueryClient();
  const privacy = useQuery({ queryKey: ["privacy"], queryFn: api.privacy });
  const sandbox = useQuery({ queryKey: ["sandbox"], queryFn: api.sandbox });
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
  const sb = sandbox.data;
  return (
    <div className="page">
      <PageTitle sub={t("settings.subtitle")}>{t("nav.settings")}</PageTitle>
      <div className="two-col">
        <Card title={t("settings.storage")}>
          <dl className="kv">
            <dt>{t("settings.language")}</dt>
            <dd>
              <LangSwitch />
            </dd>
            <dt>{t("settings.data")}</dt>
            <dd>{t("settings.dataLocal")}</dd>
            <dt>{t("settings.model")}</dt>
            <dd>{llm ? `${llm.name} · ${llm.model} · ${llm.endpoint}` : t("settings.noModel")}</dd>
            <dt>{t("settings.context")}</dt>
            <dd>{llm ? llm.remote ? <span className="warn">{t("settings.contextRemote")}</span> : t("settings.contextLocal") : "—"}</dd>
            <dt>{t("settings.sandbox")}</dt>
            <dd>{sb ? (sb.available ? `${sb.runtime} · ${t("settings.sandboxLimits", { cpus: String(sb.limits.cpus), mem: String(sb.limits.memory_mb), timeout: String(sb.limits.timeout_secs) })}` : <span className="warn">{sb.reason}</span>) : "…"}</dd>
          </dl>
          <p className="muted small">{t("settings.configHint")}</p>
        </Card>
        <Card title={t("settings.export")}>
          <p className="muted">{t("settings.exportText")}</p>
          <button className="primary" onClick={() => exp.mutate()} disabled={exp.isPending}>
            {t("settings.exportButton")}
          </button>
          <ErrorBox error={exp.error} />
        </Card>
      </div>
      <AiProviders />
      <Card title={t("settings.conversations")}>
        <p className="muted small">{t("settings.deleteHint")}</p>
        <ErrorBox error={del.error ?? conversations.error} />
        {conversations.data?.length === 0 && <Empty>{t("settings.noConversations")}</Empty>}
        <ul className="list">
          {conversations.data?.map((c) => (
            <li key={c.id}>
              <span>{c.title}</span>
              <span className="muted small">{fmt.date(c.updated_at)}</span>
              <button
                className="danger small push"
                disabled={del.isPending}
                onClick={() => {
                  if (confirm(t("settings.deleteConfirm", { title: c.title }))) del.mutate(c.id);
                }}
              >
                {t("common.delete")}
              </button>
            </li>
          ))}
        </ul>
      </Card>
    </div>
  );
}
