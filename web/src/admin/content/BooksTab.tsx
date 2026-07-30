import { useQuery } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Button } from "../../shared/ui/button";
import { field, str } from "../shared/row-utils";

function StarRating({ rating }: { rating: unknown }) {
  const r = typeof rating === "number" ? Math.floor(rating / 2) : null;
  if (r == null) return <span className="text-xs text-muted">—</span>;
  return <span className="text-[#eab308] text-xs">{"★".repeat(r)}{"☆".repeat(5 - r)}</span>;
}

export function BooksTab({ slug }: { slug: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "books"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug, "/books");
      if (!res.ok) return [];
      return ((await res.json()) as { data?: unknown[] }).data ?? [];
    },
  });

  const columns = [
    { key: "title", label: "Title" },
    {
      key: "author", label: "Author", width: "120px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "author"))}</span>,
    },
    {
      key: "rating", label: "Rating", width: "100px" as const,
      render: (row: unknown) => <StarRating rating={field(row, "rating")} />,
    },
    {
      key: "read", label: "Read", width: "100px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "read_at"))}</span>,
    },
  ];

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <input placeholder="Search books..." className="w-60 px-3 py-1.5 border border-line rounded-md text-sm bg-surface/50" />
        <Button size="sm">+ Add Review</Button>
      </div>
      <ContentTable columns={columns} data={data ?? []} isLoading={isLoading}
        emptyTitle="No book reviews yet" emptyDescription="Add your first book review." />
    </div>
  );
}
