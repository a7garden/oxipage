import { useQuery } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Badge } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";
import { field, str } from "../shared/row-utils";

export function BlogTab({ slug }: { slug: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "blog"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug, "/blog/posts");
      if (!res.ok) return [];
      return ((await res.json()) as { data?: unknown[] }).data ?? [];
    },
  });

  const columns = [
    { key: "title", label: "Title" },
    {
      key: "status",
      label: "Status",
      width: "80px" as const,
      render: (row: unknown) => (
        <Badge variant={field(row, "published_at") ? "positive" : "secondary"}>
          {field(row, "published_at") ? "Published" : "Draft"}
        </Badge>
      ),
    },
    {
      key: "lang",
      label: "Lang",
      width: "60px" as const,
      render: (row: unknown) => <span className="text-muted text-xs">{str(field(row, "lang"))}</span>,
    },
    {
      key: "updated",
      label: "Updated",
      width: "100px" as const,
      render: (row: unknown) => (
        <span className="text-muted text-xs">{str(field(row, "updated_at") || field(row, "published_at"))}</span>
      ),
    },
  ];

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <input placeholder="Search posts..." className="w-60 px-3 py-1.5 border border-line rounded-md text-sm bg-surface/50" />
        <Button size="sm">+ New Post</Button>
      </div>
      <ContentTable columns={columns} data={data ?? []} isLoading={isLoading}
        emptyTitle="No posts yet" emptyDescription="Write your first blog post." />
    </div>
  );
}
