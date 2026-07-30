import { useParams, useNavigate } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { listSites, siteScopedFetch } from "../shared/api";
import { StatCard } from "../shared/stat-card";
import { Button } from "../../shared/ui/button";
import { Badge } from "../../shared/ui/badge";
import { Skeleton } from "../../shared/ui/skeleton";
import { EmptyState, EmptyStateTitle, EmptyStateDescription } from "../../shared/ui/empty-state";
import { Plus, RefreshCw, Rocket } from "lucide-react";

export function DashboardPage() {
  const { slug } = useParams();
  const navigate = useNavigate();

  const { data: sitesData } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const site = sitesData?.data?.find((s) => s.name === slug);

  const { data: recent, isLoading, isError } = useQuery({
    queryKey: ["site", slug, "recent"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug!, "/blog/posts?limit=5");
      if (!res.ok) return [];
      const json = await res.json();
      return json.data ?? [];
    },
    enabled: !!slug,
  });

  return (
    <div>
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Dashboard</h1>
          <p className="text-sm text-muted mt-0.5">{site?.name ?? slug}</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => fetch(`/api/console/s/${slug}/build`, { method: "POST" })}>
            <RefreshCw size={14} className="mr-1" /> Rebuild
          </Button>
          <Button size="sm" onClick={() => fetch(`/api/console/s/${slug}/deploy`, { method: "POST" })}>
            <Rocket size={14} className="mr-1" /> Deploy
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-4 gap-3 mb-6">
        <StatCard label="Extensions" value={8} change="all active" />
        <StatCard label="Posts" value="—" change="coming soon" />
        <StatCard label="Storage" value="—" change="coming soon" />
        <StatCard label="Uptime" value="—" change="server online" />
      </div>

      <h2 className="text-sm font-semibold text-foreground mb-3">Recent Posts</h2>
      {isLoading ? (
        <div className="space-y-2">
          {[1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-12 w-full" />
          ))}
        </div>
      ) : isError ? (
        <div className="border border-line rounded-lg p-6 text-center text-muted text-sm">
          Failed to load recent posts.{" "}
          <button onClick={() => window.location.reload()} className="underline">
            Retry
          </button>
        </div>
      ) : recent && recent.length > 0 ? (
        <div className="border border-line rounded-lg overflow-hidden">
          <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider border-b border-line">
            <div className="flex-1 px-4 py-2.5">Title</div>
            <div className="w-20 px-4 py-2.5">Status</div>
            <div className="w-28 px-4 py-2.5">Updated</div>
          </div>
          {recent.map((post: any, i: number) => (
            <div
              key={post.id ?? post.slug ?? i}
              className="flex border-t border-line text-sm hover:bg-surface/30 cursor-pointer"
            >
              <div className="flex-1 px-4 py-2.5 truncate">{post.title}</div>
              <div className="w-20 px-4 py-2.5">
                <Badge variant={post.published_at ? "positive" : "secondary"}>
                  {post.published_at ? "Published" : "Draft"}
                </Badge>
              </div>
              <div className="w-28 px-4 py-2.5 text-muted text-xs">
                {post.published_at ?? post.updated_at ?? "—"}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <EmptyState>
          <EmptyStateTitle>No posts yet</EmptyStateTitle>
          <EmptyStateDescription>Write your first post in the Content section.</EmptyStateDescription>
        </EmptyState>
      )}

      <h2 className="text-sm font-semibold text-foreground mt-6 mb-3">Quick Actions</h2>
      <div className="flex gap-2">
        <Button variant="outline" onClick={() => navigate(`/s/${slug}/content`)}>
          <Plus size={14} className="mr-1" /> New Post
        </Button>
        <Button variant="outline" onClick={() => navigate(`/s/${slug}/extensions`)}>
          <Plus size={14} className="mr-1" /> Install Extension
        </Button>
        <Button variant="outline" onClick={() => navigate(`/s/${slug}/deploy`)}>
          <Rocket size={14} className="mr-1" /> Build & Deploy
        </Button>
      </div>
    </div>
  );
}
