import { useParams } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Skeleton } from "../../shared/ui/skeleton";

interface Extension {
  id: string;
  name?: string;
  crate_id?: string;
  enabled?: boolean;
}

export function ExtensionsPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const qc = useQueryClient();

  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "extensions"],
    queryFn: async (): Promise<{ installed: Extension[]; available: Extension[] }> => {
      const res = await siteScopedFetch(slug!, "/extensions");
      if (!res.ok) return { installed: [], available: [] };
      const json = (await res.json()) as { data?: { installed: Extension[]; available: Extension[] } };
      return json.data ?? { installed: [], available: [] };
    },
  });

  const toggleExt = useMutation({
    mutationFn: async ({ id, enable }: { id: string; enable: boolean }) => {
      const path = enable ? `/extensions/${id}/enable` : `/extensions/${id}/disable`;
      await siteScopedFetch(slug!, path);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "extensions"] }),
  });

  const installed = data?.installed ?? [];
  const available = data?.available ?? [];

  return (
    <div>
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Extensions</h1>
          <p className="text-sm text-muted mt-0.5">Manage installed extensions for {slug}</p>
        </div>
        <Button>+ Install Extension</Button>
      </div>

      {isLoading ? (
        <div className="grid grid-cols-2 gap-3">
          {[1, 2, 3, 4].map((i) => (
            <Skeleton key={i} className="h-16" />
          ))}
        </div>
      ) : (
        <>
          {installed.length > 0 && (
            <>
              <div className="text-xs font-semibold uppercase tracking-wider text-muted mb-3">
                Installed ({installed.length})
              </div>
              <div className="grid grid-cols-2 gap-3 mb-6">
                {installed.map((ext) => (
                  <div
                    key={ext.id}
                    className={`border border-line rounded-lg p-3 flex items-center gap-3 ${
                      !ext.enabled ? "opacity-50" : ""
                    }`}
                  >
                    <div className="size-9 rounded-lg bg-[#dcfce7] text-[#166534] flex items-center justify-center text-base font-bold shrink-0">
                      {(ext.name ?? ext.id)[0]?.toUpperCase() ?? "?"}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium">{ext.name ?? ext.id}</div>
                      <div className="text-xs text-muted truncate">{ext.crate_id ?? ext.id}</div>
                    </div>
                    <Button
                      variant={ext.enabled ? "outline" : "outline"}
                      size="sm"
                      className={ext.enabled ? "border-red-300 text-red-600 hover:bg-red-50" : ""}
                      onClick={() => toggleExt.mutate({ id: ext.id, enable: !ext.enabled })}
                    >
                      {ext.enabled ? "Disable" : "Enable"}
                    </Button>
                  </div>
                ))}
              </div>
            </>
          )}

          {available.length > 0 && (
            <>
              <div className="text-xs font-semibold uppercase tracking-wider text-muted mb-3">
                Available from Registry
              </div>
              <div className="grid grid-cols-2 gap-3">
                {available.map((ext) => (
                  <div
                    key={ext.id}
                    className="border border-line rounded-lg p-3 flex items-center gap-3 opacity-50"
                  >
                    <div className="size-9 rounded-lg bg-[#f0f0ee] text-[#aaa] flex items-center justify-center text-base font-bold shrink-0">
                      {(ext.name ?? ext.id)[0]?.toUpperCase() ?? "?"}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium">{ext.name ?? ext.id}</div>
                      <div className="text-xs text-muted truncate">{ext.crate_id ?? ext.id}</div>
                    </div>
                    <Button variant="outline" size="sm">
                      Install
                    </Button>
                  </div>
                ))}
              </div>
            </>
          )}

          {installed.length === 0 && available.length === 0 && (
            <div className="text-center py-12 text-muted">No extensions found.</div>
          )}
        </>
      )}
    </div>
  );
}
