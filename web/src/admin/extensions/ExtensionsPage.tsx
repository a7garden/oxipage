import { useState } from "react";
import { useParams } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { listExtensions, setExtensionEnabled, type ExtensionStatus, listRegistry, installExtension, type RegistryEntry } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Skeleton } from "../../shared/ui/skeleton";
import { ConsolePageHeader } from "../shell/ConsolePageHeader";

export function ExtensionsPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const qc = useQueryClient();

  const { data, isLoading, isError } = useQuery({
    queryKey: ["site", slug, "extensions"],
    queryFn: () => listExtensions(slug!),
    enabled: !!slug,
  });

  const toggleExt = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      setExtensionEnabled(slug!, id, enabled),
    onMutate: async ({ id, enabled }) => {
      await qc.cancelQueries({ queryKey: ["site", slug, "extensions"] });
      const prev = qc.getQueryData<ExtensionStatus[]>(["site", slug, "extensions"]);
      qc.setQueryData<ExtensionStatus[]>(["site", slug, "extensions"], (old) =>
        old?.map((e) => (e.id === id ? { ...e, enabled } : e)),
      );
      return { prev };
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(["site", slug, "extensions"], ctx.prev);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: ["site", slug, "extensions"] }),
  });

  return (
    <div>
      <ConsolePageHeader
        title="Extensions"
        description={`Manage installed extensions for ${slug}`}
      />

      {isLoading ? (
        <div className="grid grid-cols-2 gap-3">
          {[1, 2, 3, 4].map((i) => (
            <Skeleton key={i} className="h-16" />
          ))}
        </div>
      ) : isError ? (
        <div className="border border-line rounded-lg p-6 text-center text-muted text-sm">
          Failed to load extensions.{" "}
          <button onClick={() => qc.invalidateQueries({ queryKey: ["site", slug, "extensions"] })} className="underline">Retry</button>
        </div>
      ) : (
        <>
          <div className="text-xs font-semibold uppercase tracking-wider text-muted mb-3">
            Installed ({data?.length ?? 0})
          </div>
          <div className="grid grid-cols-2 gap-3 mb-6">
            {(data ?? []).map((ext) => (
              <div
                key={ext.id}
                className={`border border-line rounded-lg p-3 flex items-center gap-3 ${
                  !ext.enabled ? "opacity-60" : ""
                }`}
              >
                <div className="size-9 rounded-lg bg-positive-bg text-positive-fg flex items-center justify-center text-base font-bold shrink-0">
                  {ext.display_name[0]?.toUpperCase() ?? "?"}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium">{ext.display_name}</div>
                  <div className="text-xs text-muted truncate">{ext.id}</div>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  className={ext.enabled ? "border-destructive-border text-destructive-fg hover:bg-destructive-bg" : ""}
                  onClick={() => toggleExt.mutate({ id: ext.id, enabled: !ext.enabled })}
                  disabled={toggleExt.isPending}
                >
                  {ext.enabled ? "Disable" : "Enable"}
                </Button>
              </div>
          ))}
          </div>
          {toggleExt.isError && (
            <p className="text-sm text-destructive-fg mt-2">
              {toggleExt.error instanceof Error ? toggleExt.error.message : "Toggle failed"}
            </p>
          )}
          {(data?.length ?? 0) === 0 && (
            <div className="text-center py-12 text-muted">No extensions found.</div>
          )}
        </>
      )}

      <RegistrySection slug={slug} />
    </div>
  );
}

function RegistrySection({ slug }: { slug?: string }) {
  const qc = useQueryClient();
  const [note, setNote] = useState<string | null>(null);

  const { data: registry = [] } = useQuery({
    queryKey: ["extensions", "registry"],
    queryFn: listRegistry,
  });

  const installMut = useMutation({
    mutationFn: (name: string) => installExtension(name),
    onSuccess: (result, name) => {
      if (slug) qc.invalidateQueries({ queryKey: ["site", slug, "extensions"] });
      qc.invalidateQueries({ queryKey: ["extensions", "registry"] });
      setNote(`${name}: ${result.activated ? "Activated" : result.note ?? "Restart to activate"}`);
    },
  });

  const available = registry.filter((r) => !r.installed);
  if (available.length === 0) return null;

  return (
    <>
      <div className="text-xs font-semibold uppercase tracking-wider text-muted mb-3 mt-6">
        Available from Registry
      </div>
      <div className="grid grid-cols-2 gap-3 mb-6">
        {available.map((entry) => (
          <div key={entry.name} className="border border-line rounded-lg p-3 flex items-center gap-3">
            <div className="size-9 rounded-lg bg-info-bg text-info-fg flex items-center justify-center text-base font-bold shrink-0">
              {entry.name[0].toUpperCase()}
            </div>
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium">{entry.name}</div>
              <div className="text-xs text-muted truncate">{entry.source}</div>
            </div>
            <Button size="sm" onClick={() => installMut.mutate(entry.name)} disabled={installMut.isPending}>
              Install
            </Button>
          </div>
        ))}
      </div>
      {note && <p className="text-xs text-muted mt-2">{note}</p>}
    </>
  );
}
