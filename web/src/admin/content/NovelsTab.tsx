import { useQuery } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Button } from "../../shared/ui/button";
import { field, str } from "../shared/row-utils";

export function NovelsTab({ slug }: { slug: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "novels"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug, "/novels");
      if (!res.ok) return [];
      return ((await res.json()) as { data?: unknown[] }).data ?? [];
    },
  });

  const columns = [
    { key: "title", label: "Title" },
    {
      key: "chapters", label: "Ch.", width: "70px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "chapter_count"))}</span>,
    },
    {
      key: "char_count", label: "Chars", width: "80px" as const,
      render: (row: unknown) => {
        const c = field(row, "char_count");
        return <span className="text-xs text-muted">{typeof c === "number" ? `${(c / 1000).toFixed(0)}k` : "—"}</span>;
      },
    },
    {
      key: "updated", label: "Updated", width: "100px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "updated_at"))}</span>,
    },
  ];

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <input placeholder="Search novels..." className="w-60 px-3 py-1.5 border border-line rounded-md text-sm bg-surface/50" />
        <Button size="sm">+ New Novel</Button>
      </div>
      <ContentTable columns={columns} data={data ?? []} isLoading={isLoading}
        emptyTitle="No novels yet" emptyDescription="Start writing your first novel." />
    </div>
  );
}
