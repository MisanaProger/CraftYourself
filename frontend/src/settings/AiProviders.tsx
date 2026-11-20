import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useState } from "react";
import { useT, type Key } from "../i18n";
import { api } from "../services/api";
import type { LlmAuth, LlmModelsQuery, LlmPreset, LlmProtocol, LlmProviderView } from "../types/api";
import { Card, Empty, ErrorBox } from "../ui";

type Group = "api_key" | "subscription" | "local";
const GROUPS: { id: Group; key: Key }[] = [
  { id: "api_key", key: "ai.groupApiKey" },
  { id: "subscription", key: "ai.groupSubscription" },
  { id: "local", key: "ai.groupLocal" },
];

const groupOf = (p: LlmPreset): Group => (p.auth === "subscription" ? "subscription" : p.auth === "none" || p.id === "custom" ? "local" : "api_key");

const AUTH_KEY: Record<LlmAuth, Key> = {
  api_key: "ai.authApiKey",
  subscription: "ai.authSubscription",
  none: "ai.authNone",
};

function useInvalidate() {
  const qc = useQueryClient();
  return () => {
    qc.invalidateQueries({ queryKey: ["llm-providers"] });
    qc.invalidateQueries({ queryKey: ["privacy"] });
  };
}

/** Model id input with suggestions and a button that reads the provider's live list. */
function ModelField({ value, onChange, suggestions, query }: { value: string; onChange: (v: string) => void; suggestions: string[]; query: () => LlmModelsQuery }) {
  const { t } = useT();
  const listId = useId();
  const load = useMutation({ mutationFn: () => api.llmModels(query()) });
  const models = Array.from(new Set([...(load.data ?? []), ...suggestions]));
  return (
    <div className="stack tight">
      <div className="row">
        <input className="grow" value={value} onChange={(e) => onChange(e.target.value)} list={listId} placeholder={t("ai.model")} aria-label={t("ai.model")} spellCheck={false} />
        <button type="button" onClick={() => load.mutate()} disabled={load.isPending}>
          {load.isPending ? t("common.loading") : t("ai.loadModels")}
        </button>
      </div>
      <datalist id={listId}>
        {models.map((m) => (
          <option key={m} value={m} />
        ))}
      </datalist>
      {load.data && <span className="muted small">{t("ai.modelsFound", { n: load.data.length })}</span>}
      <ErrorBox error={load.error} />
    </div>
  );
}

function AddProvider({ presets, onDone }: { presets: LlmPreset[]; onDone: () => void }) {
  const { t } = useT();
  const invalidate = useInvalidate();
  const [group, setGroup] = useState<Group>("api_key");
  const [preset, setPreset] = useState<LlmPreset | null>(null);
  const [name, setName] = useState("");
  const [model, setModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [env, setEnv] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [protocol, setProtocol] = useState<LlmProtocol>("openai-compatible");

  useEffect(() => {
    if (!preset) return;
    setName(preset.name);
    setModel(preset.models[0] ?? "");
    setApiKey("");
    setEnv(preset.api_key_env ?? "");
    setBaseUrl(preset.base_url);
    setProtocol(preset.protocol);
  }, [preset]);

  const add = useMutation({
    mutationFn: (activate: boolean) =>
      api.addLlmProvider({
        preset: preset!.id,
        name: name.trim() || undefined,
        model: model.trim(),
        base_url: baseUrl.trim() || undefined,
        protocol: preset!.id === "custom" ? protocol : undefined,
        api_key: apiKey.trim() || undefined,
        api_key_env: env.trim(),
        activate,
      }),
    onSuccess: () => {
      setPreset(null);
      onDone();
    },
    onSettled: invalidate,
  });

  const shown = presets.filter((p) => groupOf(p) === group);
  const needsKey = preset && preset.auth !== "none";
  const local = preset?.auth === "none";
  const query = (): LlmModelsQuery => ({
    preset: preset!.id,
    base_url: baseUrl.trim() || undefined,
    protocol: preset!.id === "custom" ? protocol : undefined,
    api_key: apiKey.trim() || undefined,
    api_key_env: env.trim() || undefined,
  });

  return (
    <div className="stack">
      <div className="tabs flush" role="tablist">
        {GROUPS.map((g) => (
          <button
            key={g.id}
            type="button"
            role="tab"
            aria-selected={group === g.id}
            className={`tab ${group === g.id ? "active" : ""}`}
            onClick={() => {
              setGroup(g.id);
              setPreset(null);
            }}
          >
            {t(g.key)}
          </button>
        ))}
      </div>
      {group === "subscription" && <p className="info-box">{t("ai.subscriptionNote")}</p>}
      <div className="preset-grid">
        {shown.map((p) => (
          <button key={p.id} type="button" className={`preset ${preset?.id === p.id ? "on" : ""}`} aria-pressed={preset?.id === p.id} onClick={() => setPreset(p)}>
            <strong>{p.name}</strong>
            <span className="muted small">{p.vendor === "custom" ? t("ai.custom") : p.vendor === "local" ? t("ai.local") : p.vendor}</span>
          </button>
        ))}
      </div>

      {preset && (
        <form
          className="stack provider-form"
          onSubmit={(e) => {
            e.preventDefault();
            add.mutate(true);
          }}
        >
          <label className="field">
            <span>{t("ai.name")}</span>
            <input value={name} onChange={(e) => setName(e.target.value)} maxLength={80} />
          </label>
          {(preset.id === "custom" || !preset.base_url) && (
            <>
              <label className="field">
                <span>{t("ai.baseUrl")}</span>
                <input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="https://…/v1" spellCheck={false} required />
              </label>
              <label className="field">
                <span>{t("ai.protocol")}</span>
                <select value={protocol} onChange={(e) => setProtocol(e.target.value as LlmProtocol)}>
                  <option value="openai-compatible">OpenAI-compatible (/chat/completions)</option>
                  <option value="openai">OpenAI platform</option>
                  <option value="anthropic">Anthropic Messages API</option>
                </select>
              </label>
            </>
          )}
          {needsKey && (
            <label className="field">
              <span>
                {preset.auth === "subscription" ? t("ai.planKey") : t("ai.apiKey")}
                {preset.key_url && (
                  <>
                    {" · "}
                    <a href={preset.key_url} target="_blank" rel="noopener noreferrer">
                      {preset.auth === "subscription" ? t("ai.getPlan") : t("ai.getKey")}
                    </a>
                  </>
                )}
              </span>
              <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} autoComplete="off" spellCheck={false} placeholder={t("ai.keyPlaceholder")} />
              <span className="muted small">{t("ai.keyStorage")}</span>
            </label>
          )}
          <label className="field">
            <span>{t("ai.model")}</span>
            <ModelField value={model} onChange={setModel} suggestions={preset.models} query={query} />
          </label>
          <details>
            <summary className="muted small">{t("ai.advanced")}</summary>
            <div className="stack" style={{ marginTop: "0.6rem" }}>
              {preset.id !== "custom" && preset.base_url && (
                <label className="field">
                  <span>{t("ai.baseUrl")}</span>
                  <input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} spellCheck={false} />
                </label>
              )}
              {needsKey && (
                <label className="field">
                  <span>{t("ai.envVar")}</span>
                  <input value={env} onChange={(e) => setEnv(e.target.value.toUpperCase())} placeholder="MY_API_KEY" spellCheck={false} />
                  <span className="muted small">{t("ai.envHint")}</span>
                </label>
              )}
              <a className="small" href={preset.docs_url} target="_blank" rel="noopener noreferrer">
                {t("ai.docs")}
              </a>
            </div>
          </details>
          {!local && <p className="warn-box">{t("ai.remoteWarning", { vendor: preset.vendor === "custom" ? t("ai.custom") : preset.vendor })}</p>}
          <ErrorBox error={add.error} />
          <div className="row">
            <button className="primary" disabled={!model.trim() || add.isPending}>
              {t("ai.connect")}
            </button>
            <button type="button" disabled={!model.trim() || add.isPending} onClick={() => add.mutate(false)}>
              {t("ai.saveOnly")}
            </button>
            <button type="button" className="push" onClick={() => setPreset(null)}>
              {t("common.cancel")}
            </button>
          </div>
        </form>
      )}
    </div>
  );
}

function ProviderRow({ p, presets }: { p: LlmProviderView; presets: LlmPreset[] }) {
  const { t } = useT();
  const invalidate = useInvalidate();
  const [editing, setEditing] = useState(false);
  const [model, setModel] = useState(p.model);
  const [apiKey, setApiKey] = useState("");
  const activate = useMutation({ mutationFn: (id: string | null) => api.setActiveLlm(id), onSettled: invalidate });
  const update = useMutation({
    mutationFn: (clearKey: boolean) =>
      api.updateLlmProvider(p.id, clearKey ? { clear_key: true } : { model: model.trim(), api_key: apiKey.trim() || undefined }),
    onSuccess: () => {
      setApiKey("");
      setEditing(false);
    },
    onSettled: invalidate,
  });
  const del = useMutation({ mutationFn: () => api.deleteLlmProvider(p.id), onSettled: invalidate });
  const preset = presets.find((x) => x.id === p.preset);
  const keyLabel: Record<LlmProviderView["key"], string> = {
    stored: t("ai.keyStored"),
    env: t("ai.keyEnv", { env: p.api_key_env ?? "" }),
    missing: t("ai.keyMissing"),
    not_needed: t("ai.keyNotNeeded"),
  };
  return (
    <li className={`provider ${p.active ? "on" : ""}`}>
      <div className="provider-main">
        <div className="row">
          <strong>{p.name}</strong>
          {p.active && <span className="badge badge-active">{t("ai.active")}</span>}
          <span className="chip">{t(AUTH_KEY[p.auth])}</span>
          {p.remote ? <span className="tag warn">{t("ai.cloud")}</span> : <span className="chip">{t("ai.local")}</span>}
          {p.read_only && <span className="chip">config.toml</span>}
        </div>
        <span className="muted small mono">
          {p.model} · {p.base_url}
        </span>
        <span className={`small ${p.key === "missing" ? "warn" : "muted"}`}>{keyLabel[p.key]}</span>
      </div>
      <div className="row provider-actions">
        {p.active ? (
          <button type="button" onClick={() => activate.mutate(null)} disabled={activate.isPending}>
            {t("ai.disconnect")}
          </button>
        ) : (
          <button type="button" className="primary" onClick={() => activate.mutate(p.id)} disabled={activate.isPending}>
            {t("ai.use")}
          </button>
        )}
        {!p.read_only && (
          <>
            <button type="button" onClick={() => setEditing((e) => !e)} aria-expanded={editing}>
              {t("ai.edit")}
            </button>
            <button
              type="button"
              className="danger"
              disabled={del.isPending}
              onClick={() => {
                if (confirm(t("ai.deleteConfirm", { name: p.name }))) del.mutate();
              }}
            >
              {t("common.delete")}
            </button>
          </>
        )}
      </div>
      {editing && (
        <form
          className="stack provider-form"
          onSubmit={(e) => {
            e.preventDefault();
            update.mutate(false);
          }}
        >
          <label className="field">
            <span>{t("ai.model")}</span>
            <ModelField value={model} onChange={setModel} suggestions={preset?.models ?? []} query={() => ({ provider_id: p.id, api_key: apiKey.trim() || undefined })} />
          </label>
          {p.auth !== "none" && (
            <label className="field">
              <span>{t("ai.newKey")}</span>
              <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} autoComplete="off" spellCheck={false} placeholder={t("ai.keyPlaceholder")} />
            </label>
          )}
          <div className="row">
            <button className="primary" disabled={!model.trim() || update.isPending}>
              {t("ai.save")}
            </button>
            {p.key === "stored" && (
              <button type="button" className="danger" disabled={update.isPending} onClick={() => update.mutate(true)}>
                {t("ai.removeKey")}
              </button>
            )}
          </div>
        </form>
      )}
      <ErrorBox error={activate.error ?? update.error ?? del.error} />
    </li>
  );
}

export function AiProviders() {
  const { t } = useT();
  const presets = useQuery({ queryKey: ["llm-presets"], queryFn: api.llmPresets, staleTime: Infinity });
  const providers = useQuery({ queryKey: ["llm-providers"], queryFn: api.llmProviders });
  const [adding, setAdding] = useState(false);
  const list = providers.data?.providers ?? [];
  return (
    <Card
      title={t("ai.title")}
      actions={
        !adding && (
          <button type="button" className="primary small" onClick={() => setAdding(true)}>
            {t("ai.add")}
          </button>
        )
      }
    >
      <p className="muted small">{t("ai.intro")}</p>
      <ErrorBox error={providers.error ?? presets.error} />
      {providers.data && list.length === 0 && !adding && <Empty>{t("ai.none")}</Empty>}
      {list.length > 0 && (
        <ul className="providers">
          {list.map((p) => (
            <ProviderRow key={p.id} p={p} presets={presets.data ?? []} />
          ))}
        </ul>
      )}
      {providers.data && list.length > 0 && !providers.data.active && <p className="muted small">{t("ai.noneActive")}</p>}
      {adding && presets.data && (
        <div className="add-provider">
          <div className="row-between">
            <h3>{t("ai.addTitle")}</h3>
            <button type="button" className="icon-btn" onClick={() => setAdding(false)} aria-label={t("common.cancel")}>
              ✕
            </button>
          </div>
          <AddProvider presets={presets.data} onDone={() => setAdding(false)} />
        </div>
      )}
    </Card>
  );
}
