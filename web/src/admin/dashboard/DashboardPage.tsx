import { useParams, useNavigate } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { listSites, contentClient, triggerBuild, triggerDeploy, siteScopedFetch } from "../shared/api";
import { StatCard } from "../shared/stat-card";
import { Button } from "../../shared/ui/button";
import { Badge } from "../../shared/ui/badge";
import { Skeleton } from "../../shared/ui/skeleton";
import { EmptyState, EmptyStateTitle, EmptyStateDescription } from "../../shared/ui/empty-state";
import { Plus, RefreshCw, Rocket, Pencil, Trash2 } from "lucide-react";
import { field, str } from "../shared/row-utils";

interface BlogPost {
  id: number;
  slug: string;
  title: string;
  published_at: string | null;
  updated_at: string;
}

export function DashboardPage() {
  const { slug } = useParams();
  const navigate = useNavigate();

  const { data: sitesData } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const site = sitesData?.data?.find((s) => s.name === slug);

  const { data: posts, isLoading, isError } = useQuery({
    queryKey: ["site", slug, "blog", "recent"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug!, "/blog");
      if (!res.ok) return [];
      const json = (await res.json()) as { data?: BlogPost[] };
      return json.data ?? [];
    },
    enabled: !!slug,
  });

  const { data: counts } = useQuery({
    queryKey: ["site", slug, "counts"],
    queryFn: async () => {
      const [posts, projects, links, books, movies, novels, scraps] = await Promise.all([
        contentClient.list<unknown>(slug!, "blog").then((r) => r.length).catch(() => 0),
        contentClient.list<unknown>(slug!, "projects").then((r) => r.length).catch(() => 0),
        contentClient.list<unknown>(slug!, "links").then((r) => r.length).catch(() => 0),
        contentClient.list<unknown>(slug!, "books").then((r) => r.length).catch(() => 0),
        contentClient.list<unknown>(slug!, "movies").then((r) => r.length).catch(() => 0),
        contentClient.list<unknown>(slug!, "novels").then((r) => r.length).catch(() => 0),
        contentClient.list<unknown>(slug!, "scraps").then((r) => r.length).catch(() => 0),
      ]);
      return { posts, projects, links, books, movies, novels, scraps };
    },
    enabled: !!slug,
  });

  const handleBuild = async () => {
    try {
      await triggerBuild(slug!);
      navigate(`/s/${slug}/deploy`);
    } catch (e) {
      alert(e instanceof Error ? e.message : "Build failed");
    }
  };

  const handleDeploy = async () => {
    try {
      await triggerDeploy(slug!);
      alert("Deploy queued");
    } catch (e) {
      alert(e instanceof Error ? e.message : "Deploy failed");
    }
  };

  const recent = (posts ?? []).slice(0, 5);

  return (
    <div>
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Dashboard</h1>
          <p className="text-sm text-muted mt-0.5">{site?.name ?? slug}</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={handleBuild}>
            <RefreshCw size={14} className="mr-1" /> Rebuild
          </Button>
          <Button size="sm" onClick={handleDeploy}>
            <Rocket size={14} className="mr-1" /> Deploy
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-4 gap-3 mb-6">
        <StatCard label="Posts" value={counts?.posts ?? "—"} change="blog" />
        <StatCard label="Projects" value={counts?.projects ?? "—"} change="projects" />
        <StatCard label="Books" value={counts?.books ?? "—"} change="books" />
        <StatCard label="Links" value={counts?.links ?? "—"} change="links" />
      </div>

      <h2 className="text-sm font-semibold text-foreground mb-3">Recent Posts</h2>
      {isLoading ? (
        <div className="space-y-2">
          {[1, 2, 3].map((i) => <Skeleton key={i} className="h-12 w-full" />)}
        </div>
      ) : isError ? (
        <div className="border border-line rounded-lg p-6 text-center text-muted text-sm">
          Failed to load recent posts.{" "}
          <button onClick={() => window.location.reload()} className="underline">Retry</button>
        </div>
      ) : recent.length > 0 ? (
        <div className="border border-line rounded-lg overflow-hidden">
          <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider border-b border-line">
            <div className="flex-1 px-4 py-2.5">Title</div>
            <div className="w-44 px-4 py-2.5">Status</div>
            <div className="w-28 px-4 py-2.5">Updated</div>
            <div className="w-24 px-4 py-2.5 text-right">Actions</div>
          </div>
          {recent.map((post) => (
            <div key={post.slug} className="flex border-t border-line text-sm hover:bg-surface/30">
              <div className="flex-1 px-4 py-2.5 truncate">{post.title}</div>
              <div className="w-44 px-4 py-2.5">
                <Badge variant={post.published_at ? "positive" : "secondary"}>
                  {post.published_at ? "Published" : "Draft"}
                </Badge>
              </div>
              <div className="w-28 px-4 py-2.5 text-muted text-xs">{str(post.updated_at)}</div>
              <div className="w-24 px-4 py-2.5 flex justify-end gap-1">
                <button
                  onClick={() => navigate(`/s/${slug}/content`)}
                  className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-foreground hover:bg-surface/50"
                  aria-label="Edit"
                >
                  <Pencil size={14} />
                </button>
                <button
                  onClick={() => navigate(`/s/${slug}/content`)}
                  className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-red-600 hover:bg-red-50"
                  aria-label="Delete"
                >
                  <Trash2 size={14} />
                </button>
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
        <Button variant="outline" onClick={() => navigate(`/s/${slug}/content`)}><Plus size={14} className="mr-1" /> New Post</Button>
        <Button variant="outline" onClick={() => navigate(`/s/${slug}/extensions`)}><Plus size={14} className="mr-1" /> Install Extension</Button>
        <Button variant="outline" onClick={handleBuild}><Rocket size={14} className="mr-1" /> Build & Deploy</Button>
      </div>
    </div>
  );
}
