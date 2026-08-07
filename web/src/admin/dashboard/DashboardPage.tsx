import { useParams, useNavigate } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { listSites, getStats, getRecent, type StatsResponse, type RecentItem } from "../shared/api";
import { StatCard } from "../shared/stat-card";
import { Button } from "../../shared/ui/button";
import { Badge } from "../../shared/ui/badge";
import { Skeleton } from "../../shared/ui/skeleton";
import { EmptyState, EmptyStateTitle, EmptyStateDescription } from "../../shared/ui/empty-state";
import { Plus, RefreshCw, Rocket, Pencil } from "lucide-react";
import { str } from "../shared/row-utils";
import { ConsolePageHeader } from "../shell/ConsolePageHeader";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const v = bytes / 1024 ** i;
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function DashboardPage() {
  const { slug } = useParams();
  const navigate = useNavigate();

  const { data: sitesData } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const site = sitesData?.data?.find((s) => s.name === slug);

  const {
    data: stats,
    isLoading: statsLoading,
    isError: statsError,
  } = useQuery({
    queryKey: ["site", slug, "stats"],
    queryFn: () => getStats(slug!),
    enabled: !!slug,
  });

  const {
    data: recent,
    isLoading: recentLoading,
    isError: recentError,
  } = useQuery({
    queryKey: ["site", slug, "recent"],
    queryFn: () => getRecent(slug!),
    enabled: !!slug,
  });

  const handleBuild = async () => {
    try {
      const { triggerBuild } = await import("../shared/api");
      await triggerBuild(slug!);
      navigate(`/s/${slug}/deploy`);
    } catch (e) {
      alert(e instanceof Error ? e.message : "Build failed");
    }
  };

  const handleDeploy = async () => {
    try {
      const { triggerDeploy } = await import("../shared/api");
      await triggerDeploy(slug!);
      alert("Deploy queued");
    } catch (e) {
      alert(e instanceof Error ? e.message : "Deploy failed");
    }
  };

  const statCards: { label: string; value: string | number; change?: string }[] = [];
  if (stats) {
    const labelMap: Record<string, string> = {
      blog: "Posts",
      projects: "Projects",
      books: "Books",
      movies: "Movies",
      novels: "Novels",
      links: "Links",
      scraps: "Scraps",
    };
    for (const [ext, count] of Object.entries(stats.counts)) {
      const label = labelMap[ext] ?? ext;
      statCards.push({ label, value: count, change: ext });
    }
    statCards.push({
      label: "Storage",
      value: formatBytes(stats.storage_bytes),
    });
    if (stats.last_build) {
      statCards.push({
        label: "Last Build",
        value: stats.last_build.status === "success" ? "Success" : stats.last_build.status,
        change: str(stats.last_build.started_at),
      });
    }
    if (stats.last_deploy) {
      statCards.push({
        label: "Last Deploy",
        value: stats.last_deploy.status,
        change: `${stats.last_deploy.owner}/${stats.last_deploy.repo}`,
      });
    }
  }

  return (
    <div>
      <ConsolePageHeader
        title="Dashboard"
        description={site?.name ?? slug}
        actions={
          <div className="flex gap-2">
            {stats?.last_build && (
              <Badge variant={stats.last_build.status === "success" ? "positive" : "secondary"}>
                {stats.last_build.status}
              </Badge>
            )}
            {stats?.last_deploy && (
              <Badge
                variant={stats.last_deploy.status === "failed" ? "warning" : "positive"}
                title={`${stats.last_deploy.owner}/${stats.last_deploy.repo} · ${stats.last_deploy.started_at}`}
              >
                deploy:{stats.last_deploy.status}
              </Badge>
            )}
            <Button variant="outline" size="sm" onClick={handleBuild}>
              <RefreshCw size={14} className="mr-1" /> Rebuild
            </Button>
            <Button size="sm" onClick={handleDeploy}>
              <Rocket size={14} className="mr-1" /> Deploy
            </Button>
          </div>
        }
      />

      {statsLoading ? (
        <div className="grid grid-cols-4 gap-3 mb-6">
          {[1, 2, 3, 4].map((i) => (
            <Skeleton key={i} className="h-24 w-full" />
          ))}
        </div>
      ) : statsError ? (
        <div className="border border-line rounded-lg p-6 text-center text-muted text-sm mb-6">
          Failed to load stats.{" "}
          <button
            onClick={() => window.location.reload()}
            className="underline"
          >
            Retry
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-4 gap-3 mb-6">
          {statCards.map((c) => (
            <StatCard
              key={c.label}
              label={c.label}
              value={c.value}
              change={c.change}
            />
          ))}
        </div>
      )}

      <h2 className="text-sm font-semibold text-foreground mb-3">Recent Content</h2>
      {recentLoading ? (
        <div className="space-y-2">
          {[1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-12 w-full" />
          ))}
        </div>
      ) : recentError ? (
        <div className="border border-line rounded-lg p-6 text-center text-muted text-sm">
          Failed to load recent content.{" "}
          <button
            onClick={() => window.location.reload()}
            className="underline"
          >
            Retry
          </button>
        </div>
      ) : recent && recent.length > 0 ? (
        <div className="border border-line rounded-lg overflow-hidden">
          <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider border-b border-line">
            <div className="w-20 px-4 py-2.5">Extension</div>
            <div className="flex-1 px-4 py-2.5">Title</div>
            <div className="w-28 px-4 py-2.5">Status</div>
            <div className="w-28 px-4 py-2.5">Updated</div>
            <div className="w-24 px-4 py-2.5 text-right">Actions</div>
          </div>
          {recent.map((item: RecentItem) => (
            <div
              key={`${item.ext}-${item.id}`}
              className="flex border-t border-line text-sm hover:bg-surface/30"
            >
              <div className="w-20 px-4 py-2.5">
                <Badge variant="secondary">{item.ext}</Badge>
              </div>
              <div className="flex-1 px-4 py-2.5 truncate">{item.title}</div>
              <div className="w-28 px-4 py-2.5">
                <Badge variant={item.published_at ? "positive" : "secondary"}>
                  {item.published_at ? "Published" : "Draft"}
                </Badge>
              </div>
              <div className="w-28 px-4 py-2.5 text-muted text-xs">{str(item.updated_at)}</div>
              <div className="w-24 px-4 py-2.5 flex justify-end">
                <button
                  onClick={() => navigate(`/s/${slug}/content`)}
                  className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-foreground hover:bg-surface/50"
                  aria-label="Edit"
                >
                  <Pencil size={14} />
                </button>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <EmptyState>
          <EmptyStateTitle>No content yet</EmptyStateTitle>
          <EmptyStateDescription>
            Write your first post or add content in the Content section.
          </EmptyStateDescription>
        </EmptyState>
      )}

      <h2 className="text-sm font-semibold text-foreground mt-6 mb-3">Quick Actions</h2>
      <div className="flex gap-2">
        <Button
          variant="outline"
          onClick={() => navigate(`/s/${slug}/content`)}
        >
          <Plus size={14} className="mr-1" /> New Post
        </Button>
        <Button
          variant="outline"
          onClick={() => navigate(`/s/${slug}/extensions`)}
        >
          <Plus size={14} className="mr-1" /> Install Extension
        </Button>
        <Button variant="outline" onClick={handleBuild}>
          <Rocket size={14} className="mr-1" /> Build & Deploy
        </Button>
      </div>
    </div>
  );
}
