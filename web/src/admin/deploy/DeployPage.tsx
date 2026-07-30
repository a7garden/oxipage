import { useState } from "react";
import { useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Badge } from "../../shared/ui/badge";
import { Skeleton } from "../../shared/ui/skeleton";

interface Build {
  id?: string | number;
  duration?: string;
  status?: string;
  created_at?: string;
}

export function DeployPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const [buildLog] = useState<string[]>([]);

  const { data: builds, isLoading } = useQuery({
    queryKey: ["site", slug, "builds"],
    queryFn: async (): Promise<Build[]> => {
      const res = await siteScopedFetch(slug!, "/builds");
      if (!res.ok) return [];
      return ((await res.json()) as { data?: Build[] }).data ?? [];
    },
  });

  return (
    <div>
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Deploy</h1>
          <p className="text-sm text-muted mt-0.5">Build static site and deploy to production</p>
        </div>
        <Button
          onClick={() => fetch(`/api/console/s/${slug}/build`, { method: "POST" })}
        >
          ⇧ Build & Deploy
        </Button>
      </div>

      <h2 className="text-sm font-semibold text-foreground mb-3">Last Build</h2>
      {isLoading ? (
        <Skeleton className="h-32 w-full" />
      ) : builds && builds.length > 0 ? (
        <div className="border border-line rounded-lg p-5 mb-6">
          <div className="flex items-center justify-between mb-3">
            <span className="font-semibold">Build #{builds[0].id}</span>
            <Badge variant="positive">
              {builds[0].status === "deployed" ? "✓ Deployed" : builds[0].status}
            </Badge>
          </div>
          {builds[0].duration && (
            <div className="flex gap-2 items-center text-xs text-muted mb-4">
              <span className="flex items-center gap-1">
                <span className="size-2 rounded-full bg-[#22c55e]" /> Build ({builds[0].duration})
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
          )}
          {buildLog.length > 0 && (
            <div
              className="bg-[#1a1e24] text-[#a0a0a0] rounded-md p-3 font-mono text-xs leading-relaxed max-h-32 overflow-y-auto"
            >
              {buildLog.map((line, i) => (
                <div key={i}>{line}</div>
              ))}
            </div>
          )}
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
          <div className="w-24 px-4 py-2.5">Duration</div>
          <div className="w-24 px-4 py-2.5">Status</div>
          <div className="w-32 px-4 py-2.5">Time</div>
        </div>
          {builds && builds.length > 0 ? (
          builds.map((build) => (
            <div key={String(build.id)} className="flex border-t border-line text-sm">
              <div className="flex-1 px-4 py-2.5">#{build.id}</div>
              <div className="w-24 px-4 py-2.5 text-muted">{build.duration ?? "—"}</div>
              <div className="w-24 px-4 py-2.5">
                <Badge variant={build.status === "deployed" ? "positive" : "secondary"}>
                  {build.status}
                </Badge>
              </div>
              <div className="w-32 px-4 py-2.5 text-muted text-xs">
                {build.created_at ?? "—"}
              </div>
            </div>
          ))
        ) : (
          <div className="p-8 text-center text-muted text-sm">No build history</div>
        )}
      </div>
    </div>
  );
}
