import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { ErrorBox } from "../../components/ui";
import { api } from "../../services/api";

export function ImportExerciseForm() {
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
      <textarea
        className="mono"
        rows={6}
        value={manifest}
        onChange={(e) => setManifest(e.target.value)}
        placeholder={"manifest_version = 1\n[exercise]\n…"}
        aria-label="Exercise Manifest (TOML)"
      />
      <label className="file">
        или файл .toml
        <input type="file" accept=".toml" onChange={async (e) => setManifest((await e.target.files?.[0]?.text()) ?? "")} />
      </label>
      <div className="row">
        <button disabled={!manifest.trim() || imp.isPending}>Импортировать</button>
        {imp.isSuccess && <span className="ok">Задача «{imp.data.title}» добавлена</span>}
      </div>
      <ErrorBox error={imp.error} />
    </form>
  );
}
