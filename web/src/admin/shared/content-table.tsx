import type { ReactNode } from "react";
import { EmptyState, EmptyStateTitle, EmptyStateDescription } from "../../shared/ui/empty-state";
import { Skeleton } from "../../shared/ui/skeleton";

interface Column {
  key: string;
  label: string;
  width?: string;
  render?: (row: unknown) => ReactNode;
}

interface ContentTableProps {
  columns: Column[];
  data: unknown[];
  isLoading: boolean;
  emptyTitle?: string;
  emptyDescription?: string;
}

export function ContentTable({
  columns,
  data,
  isLoading,
  emptyTitle,
  emptyDescription,
}: ContentTableProps) {
  if (isLoading) {
    return (
      <div className="space-y-2">
        {[1, 2, 3].map((i) => (
          <Skeleton key={i} className="h-12 w-full" />
        ))}
      </div>
    );
  }

  if (data.length === 0) {
    return (
      <EmptyState>
        <EmptyStateTitle>{emptyTitle ?? "No content"}</EmptyStateTitle>
        <EmptyStateDescription>
          {emptyDescription ?? "Create your first item."}
        </EmptyStateDescription>
      </EmptyState>
    );
  }

  return (
    <div className="border border-line rounded-lg overflow-hidden">
      <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider border-b border-line">
        {columns.map((col) => (
          <div
            key={col.key}
            className="px-4 py-2.5 truncate"
            style={{ flex: col.width ? `0 0 ${col.width}` : 1 }}
          >
            {col.label}
          </div>
        ))}
      </div>
      {data.map((row, i) => {
        const r = row as Record<string, unknown>;
        const id = (r.id ?? r.slug ?? i) as string | number;
        return (
          <div key={id} className="flex border-t border-line text-sm hover:bg-surface/30">
            {columns.map((col) => (
              <div
                key={col.key}
                className="px-4 py-2.5 truncate"
                style={{ flex: col.width ? `0 0 ${col.width}` : 1 }}
              >
                {col.render ? col.render(row) : String(r[col.key] ?? "—")}
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}
