import { useParams } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { listBuilds, triggerBuild, triggerDeploy, type BuildRecord } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Badge } from "../../shared/ui/badge";
import { Skeleton } from "../../shared/ui/skeleton";

export function DeployPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const qc = useQueryClient();

  const { data, isLoading, isError } = useQuery({
    queryKey: ["site", slug, "builds"],
    queryFn: () => listBuilds(slug!),
    enabled: !!slug,
  });

  const build = useMutation({
    mutationFn: () => triggerBuild(slug!),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "builds"] }),
  });

  const deploy = useMutation({
    mutationFn: () => triggerDeploy(slug!),
  });

  const builds: BuildRecord[] = data?.data ?? [];
  const last = builds[0];

  return (
    <div>
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Deploy</h1>
          <p className="text-sm text-muted mt-0.5">Build static site and deploy to production</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={() => build.mutate()}
            disabled={build.isPending}
          >
            {build.isPending ? "Building..." : "↧ Build"}
          </Button>
          <Button
            onClick={() => deploy.mutate()}
            disabled={deploy.isPending}
          >
            {deploy.isPending ? "Deploying..." : "⇧ Deploy"}
          </Button>
        </div>
      </div>

      {build.isError && (
        <p className="text-sm text-red-600 mb-4">
          {build.error instanceof Error ? build.error.message : "Build failed"}
        </p>
      )}
      {deploy.isError && (
        <p className="text-sm text-red-600 mb-4">
          {deploy.error instanceof Error ? deploy.error.message : "Deploy failed"}
        </p>
      )}

      <h2 className="text-sm font-semibold text-foreground mb-3">Last Build</h2>
      {isLoading ? (
        <Skeleton className="h-32 w-full" />
      ) : last ? (
        <div className="border border-line rounded-lg p-5 mb-6">
          <div className="flex items-center justify-between mb-3">
            <span className="font-semibold">Build #{last.id}</span>
            <Badge variant="positive">{last.status}</Badge>
          </div>
          <div className="flex gap-2 items-center text-xs text-muted mb-4">
            <span className="flex items-center gap-1">
              <span className="size-2 rounded-full bg-[#22c55e]" /> Build
            </span>
            <span className="text-line">→</span>
            <span className="flex items-center gap-1">
              <span className="size-2 rounded-full bg-[#22c55e]" /> Generate
            </span>
            <span className="text-line">→</span>
            <span className="flex items-center gap-1">
              <span className="size-2 rounded-full bg-[#22c55e]" /> Deploy
            </span>
          </div>
          <div className="text-xs text-muted">
            <span className="font-mono">{last.out_dir ?? "—"}</span>
            {last.page_count != null && (
              <span className="ml-3">· {last.page_count} pages</span>
            )}
          </div>
        </div>
      ) : (
        <div className="border border-line rounded-lg p-8 text-center text-muted text-sm mb-6">
          No builds yet. Trigger your first build.
        </div>
      )}

      <h2 className="text-sm font-semibold text-foreground mb-3">Build History</h2>
      <div className="border border-line rounded-lg overflow-hidden">
        <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider">
          <div className="flex-1 px-4 py-2.5">Build</div>
          <div className="w-24 px-4 py-2.5">Pages</div>
          <div className="w-24 px-4 py-2.5">Status</div>
          <div className="w-40 px-4 py-2.5">Time</div>
        </div>
        {isLoading ? (
          <div className="p-4"><Skeleton className="h-6 w-full" /></div>
        ) : isError ? (
          <div className="p-8 text-center text-muted text-sm">
            Failed to load build history.{" "}
            <button onClick={() => qc.invalidateQueries({ queryKey: ["site", slug, "builds"] })} className="underline">Retry</button>
          </div>
        ) : builds.length > 0 ? (
          builds.map((b) => (
            <div key={b.id} className="flex border-t border-line text-sm">
              <div className="flex-1 px-4 py-2.5">#{b.id}</div>
              <div className="w-24 px-4 py-2.5 text-muted">{b.page_count ?? "—"}</div>
              <div className="w-24 px-4 py-2.5">
                <Badge variant="positive">{b.status}</Badge>
              </div>
              <div className="w-40 px-4 py-2.5 text-muted text-xs">{b.created_at}</div>
            </div>
          ))
        ) : (
          <div className="p-8 text-center text-muted text-sm">No build history</div>
        )}
      </div>
    </div>
  );
}
