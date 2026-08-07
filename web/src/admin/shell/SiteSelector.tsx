import { useState, useRef, useEffect } from "react";
import { useParams, Link } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { listSites, getStats } from "../shared/api";
import { ChevronDown } from "lucide-react";

/**
 * Console site selector — the dropdown that appears in the topbar.
 *
 * Tone tokens:
 *  - bg-active            : active site indicator dot
 *  - bg-destructive-fg   : inactive site indicator
 *  - border-active        : active row hairline
 *  - text-active          : active row check mark
 */
export function SiteSelector() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const { slug } = useParams();
  const { data } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const sites = data?.data ?? [];
  const current = sites.find((s) => slug ? s.name === slug : s.active) ?? sites[0];

  // All hooks MUST run unconditionally. The early `return null` below
  // previously sat BEFORE this useQuery, so once the sites list loaded the
  // hook count rose mid-component — "rendered more hooks than the previous
  // render" (React #310), which broke every admin page. Gate the fetch via
  // `enabled` instead of branching the hook itself.
  const { data: stats } = useQuery({
    queryKey: ["site", current?.name ?? "_", "stats"],
    queryFn: () => getStats(current!.name),
    enabled: open && !!current,
  });

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  if (!current) return null;

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 px-2.5 py-1 border border-line rounded-md text-sm font-medium bg-surface/50 hover:bg-surface min-w-[140px]"
      >
        <span className="size-2 rounded-full bg-active shrink-0" />
        <span>{current.name}</span>
        <ChevronDown size={12} className="ml-auto text-muted" />
      </button>

      {open && (
        <div className="absolute top-full left-0 mt-1 w-[420px] bg-canvas border border-line rounded-lg shadow-xl z-50 p-3">
          <div className="text-[11px] font-semibold uppercase tracking-wide text-muted mb-2">Your Sites</div>
          {sites.map((s) => (
            <Link
              key={s.name}
              to={`/s/${s.name}`}
              onClick={() => setOpen(false)}
              className={`flex items-center gap-2.5 px-3 py-2 rounded-md text-sm hover:bg-surface ${
                s.name === current.name ? "border border-active bg-active-soft" : "border border-transparent"
              }`}
            >
              <span
                className={`size-2 rounded-full shrink-0 ${
                  s.active ? "bg-active" : "bg-destructive-fg"
                }`}
              />
              <div>
                <div className="font-medium">{s.name}</div>
                <div className="text-xs text-muted">{s.path}</div>
              </div>
              {s.name === current.name && (
                <span className="ml-auto text-sm text-active font-bold">✓</span>
              )}
            </Link>
          ))}
          {stats && (
            <div className="mt-3 pt-3 border-t border-line text-xs text-muted space-y-1">
              <div className="flex justify-between"><span>Content</span><span>{Object.values(stats.counts).reduce((a: number, b: number) => a + b, 0)} entries</span></div>
              <div className="flex justify-between"><span>Storage</span><span>{stats.storage_bytes < 1024 ? `${stats.storage_bytes} B` : stats.storage_bytes < 1048576 ? `${(stats.storage_bytes / 1024).toFixed(1)} KB` : `${(stats.storage_bytes / 1048576).toFixed(1)} MB`}</span></div>
              <div className="flex justify-between"><span>Last build</span><span>{stats.last_build ? new Date(stats.last_build.started_at).toLocaleDateString() : "Never"}</span></div>
            </div>
          )}
          <div className="flex gap-2 mt-3 pt-3 border-t border-line">
            <Link to="/sites" className="text-xs px-3 py-1.5 rounded border border-line hover:bg-surface" onClick={() => setOpen(false)}>Manage Sites</Link>
            <Link to="/sites/new" className="text-xs px-3 py-1.5 rounded border border-line hover:bg-surface" onClick={() => setOpen(false)}>+ Add New Site</Link>
          </div>
        </div>
      )}
    </div>
  );
}
