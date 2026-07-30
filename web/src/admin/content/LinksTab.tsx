import { useQuery } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Button } from "../../shared/ui/button";
import { field, str } from "../shared/row-utils";

export function LinksTab({ slug }: { slug: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "links"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug, "/links");
      if (!res.ok) return [];
      return ((await res.json()) as { data?: unknown[] }).data ?? [];
    },
  });

  const columns = [
    { key: "title", label: "Title" },
    {
      key: "url", label: "URL", width: "200px" as const,
      render: (row: unknown) => <span className="text-xs text-muted truncate block">{str(field(row, "url"))}</span>,
    },
    {
      key: "display_order", label: "Order", width: "60px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "display_order"))}</span>,
    },
    {
      key: "updated", label: "Updated", width: "100px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "updated_at"))}</span>,
    },
  ];

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <input placeholder="Search links..." className="w-60 px-3 py-1.5 border border-line rounded-md text-sm bg-surface/50" />
        <Button size="sm">+ New Link</Button>
      </div>
      <ContentTable columns={columns} data={data ?? []} isLoading={isLoading}
        emptyTitle="No links yet" emptyDescription="Add your first link." />
    </div>
  );
}
