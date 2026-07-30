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

export function MoviesTab({ slug }: { slug: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "movies"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug, "/movies");
      if (!res.ok) return [];
      return ((await res.json()) as { data?: unknown[] }).data ?? [];
    },
  });

  const columns = [
    {
      key: "title", label: "Title",
      render: (row: unknown) => {
        const title = str(field(row, "title"));
        const year = field(row, "year");
        return <span>{title}{year ? <span className="text-muted text-xs ml-1">({str(year)})</span> : null}</span>;
      },
    },
    {
      key: "rating", label: "Rating", width: "100px" as const,
      render: (row: unknown) => <StarRating rating={field(row, "rating")} />,
    },
    {
      key: "series", label: "Series", width: "100px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "series"))}</span>,
    },
    {
      key: "watched", label: "Watched", width: "100px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "watched_at") || field(row, "published_at"))}</span>,
    },
  ];

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <input placeholder="Search movies..." className="w-60 px-3 py-1.5 border border-line rounded-md text-sm bg-surface/50" />
        <div className="flex gap-2">
          <Button variant="outline" size="sm">+ New Series</Button>
          <Button size="sm">+ Add Review</Button>
        </div>
      </div>
      <ContentTable columns={columns} data={data ?? []} isLoading={isLoading}
        emptyTitle="No movie reviews yet" emptyDescription="Add your first movie review." />
    </div>
  );
}
