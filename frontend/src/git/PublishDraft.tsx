import { useMutation } from "@tanstack/react-query";
import { useT } from "../i18n";
import { api } from "../services/api";
import type { PublishPreview } from "../types/api";
import { ErrorBox, tv } from "../ui";

/**
 * A draft the server prepared: exactly what goes where. Only "Publish"
 * sends it, once; the server checks the digest and the time limit.
 */
export function PublishDraft({ draft, onClose, onPublished }: { draft: PublishPreview; onClose: () => void; onPublished: (url: string) => void }) {
  const { t } = useT();
  const publish = useMutation({
    mutationFn: () => api.confirmPublish(draft.id, draft.digest),
    onSuccess: (r) => {
      onPublished(r.url);
      onClose();
    },
  });
  const discard = useMutation({ mutationFn: () => api.discardDraft(draft.id), onSettled: onClose });
  return (
    <div className="stack publish-draft">
      <p className="small">
        {tv(t, "publish.kind", draft.kind)} · {t("publish.target", { host: draft.host })}
      </p>
      <a className="small" href={draft.target} target="_blank" rel="noreferrer noopener">
        {draft.target}
      </a>
      <pre>{draft.body}</pre>
      {draft.warnings.map((w) => (
        <p key={w} className="small">
          ! {tv(t, "publish.warning", w)}
        </p>
      ))}
      <div className="row">
        <button className="primary small" disabled={publish.isPending} onClick={() => publish.mutate()}>
          {t("publish.confirm")}
        </button>
        <button className="secondary small" disabled={discard.isPending || publish.isPending} onClick={() => discard.mutate()}>
          {t("publish.discard")}
        </button>
      </div>
      <ErrorBox error={publish.error ?? discard.error} />
    </div>
  );
}

/** Link to what was published. */
export function Published({ url }: { url: string | null }) {
  const { t } = useT();
  if (!url) return null;
  return (
    <p className="ok small">
      {t("publish.done")}{" "}
      <a href={url} target="_blank" rel="noreferrer noopener">
        {url}
      </a>
    </p>
  );
}
