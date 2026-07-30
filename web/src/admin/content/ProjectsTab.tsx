import { useQuery } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Badge } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";
import { field, str } from "../shared/row-utils";

export function ProjectsTab({ slug }: { slug: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "projects"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug, "/projects");
      if (!res.ok) return [];
      return ((await res.json()) as { data?: unknown[] }).data ?? [];
    },
  });

  const columns = [
    { key: "title", label: "Title" },
    {
      key: "status", label: "Status", width: "80px" as const,
      render: (row: unknown) => <Badge variant={str(field(row, "status")) === "active" ? "positive" : "secondary"}>{str(field(row, "status"))}</Badge>,
    },
    {
      key: "tech_stack", label: "Tech", width: "140px" as const,
      render: (row: unknown) => {
        const tech = field(row, "tech_stack");
        return <span className="text-xs text-muted">{Array.isArray(tech) ? tech.join(", ") : str(tech)}</span>;
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
        <input placeholder="Search projects..." className="w-60 px-3 py-1.5 border border-line rounded-md text-sm bg-surface/50" />
        <Button size="sm">+ New Project</Button>
      </div>
      <ContentTable columns={columns} data={data ?? []} isLoading={isLoading}
        emptyTitle="No projects yet" emptyDescription="Add your first project." />
    </div>
  );
}
