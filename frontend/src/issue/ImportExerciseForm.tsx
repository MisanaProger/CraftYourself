import { useT } from "../i18n";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../services/api";
import { ErrorBox } from "../ui";

export function ImportExerciseForm() {
  const { t } = useT();
  const qc = useQueryClient();
  const [manifest, setManifest] = useState("");
  const imp = useMutation({
    mutationFn: api.importExercise,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["projects"] });
      setManifest("");
    },
  });
  return (
    <form
      className="stack"
      onSubmit={(e) => {
        e.preventDefault();
        imp.mutate(manifest);
      }}
    >
      <textarea className="mono" rows={5} value={manifest} onChange={(e) => setManifest(e.target.value)} placeholder={"manifest_version = 1\n[exercise]\n…"} aria-label="Exercise Manifest (TOML)" />
      <div className="row">
        <label className="file">
          {t("import.file")}
          <input type="file" accept=".toml" onChange={async (e) => setManifest((await e.target.files?.[0]?.text()) ?? "")} />
        </label>
        <button className="primary" disabled={!manifest.trim() || imp.isPending}>
          {t("import.submit")}
        </button>
      </div>
      {imp.isSuccess && <p className="ok small">{t("import.done", { title: imp.data.title })}</p>}
      <ErrorBox error={imp.error} />
    </form>
  );
}
