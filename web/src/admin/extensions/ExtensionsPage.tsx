import { useParams } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { listExtensions, setExtensionEnabled, type ExtensionStatus } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Skeleton } from "../../shared/ui/skeleton";

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
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Extensions</h1>
          <p className="text-sm text-muted mt-0.5">Manage installed extensions for {slug}</p>
        </div>
      </div>

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
                <div className="size-9 rounded-lg bg-[#dcfce7] text-[#166534] flex items-center justify-center text-base font-bold shrink-0">
                  {ext.display_name[0]?.toUpperCase() ?? "?"}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium">{ext.display_name}</div>
                  <div className="text-xs text-muted truncate">{ext.id}</div>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  className={ext.enabled ? "border-red-300 text-red-600 hover:bg-red-50" : ""}
                  onClick={() => toggleExt.mutate({ id: ext.id, enabled: !ext.enabled })}
                  disabled={toggleExt.isPending}
                >
                  {ext.enabled ? "Disable" : "Enable"}
                </Button>
              </div>
            ))}
          </div>
          {toggleExt.isError && (
            <p className="text-sm text-red-600 mt-2">
              {toggleExt.error instanceof Error ? toggleExt.error.message : "Toggle failed"}
            </p>
          )}
          {(data?.length ?? 0) === 0 && (
            <div className="text-center py-12 text-muted">No extensions found.</div>
          )}
        </>
      )}
    </div>
  );
}
