import { useQuery } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Badge } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";
import { field, str } from "../shared/row-utils";

export function ScrapsTab({ slug }: { slug: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "scraps"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug, "/scraps");
      if (!res.ok) return [];
      return ((await res.json()) as { data?: unknown[] }).data ?? [];
    },
  });

  const columns = [
    { key: "title", label: "Title" },
    {
      key: "source", label: "Source", width: "80px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "source"))}</span>,
    },
    {
      key: "status", label: "Status", width: "80px" as const,
      render: (row: unknown) => (
        <Badge variant={field(row, "published_at") ? "positive" : "secondary"}>
          {field(row, "published_at") ? "Published" : "Queued"}
        </Badge>
      ),
    },
    {
      key: "collected", label: "Collected", width: "100px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "created_at"))}</span>,
    },
  ];

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <input placeholder="Search scraps..." className="w-60 px-3 py-1.5 border border-line rounded-md text-sm bg-surface/50" />
        <Button size="sm">+ Add Scrap</Button>
      </div>
      <ContentTable columns={columns} data={data ?? []} isLoading={isLoading}
        emptyTitle="No scraps yet" emptyDescription="Collect your first scrap." />
    </div>
  );
}
