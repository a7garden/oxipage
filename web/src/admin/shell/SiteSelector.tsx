import { useState, useRef, useEffect } from "react";
import { useParams, Link } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { listSites } from "../shared/api";
import { ChevronDown } from "lucide-react";

export function SiteSelector() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const { slug } = useParams();
  const { data } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const sites = data?.data ?? [];

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  const current = sites.find((s) => slug ? s.name === slug : s.active) ?? sites[0];
  if (!current) return null;

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 px-2.5 py-1 border border-line rounded-md text-sm font-medium bg-surface/50 hover:bg-surface min-w-[140px]"
      >
        <span className="size-2 rounded-full bg-[#22c55e] shrink-0" />
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
              className="flex items-center gap-2.5 px-3 py-2 rounded-md text-sm hover:bg-surface"
              style={s.name === current.name ? { backgroundColor: "rgba(34,197,94,0.08)" } : {}}
            >
              <span className={`size-2 rounded-full shrink-0 ${s.active ? "bg-[#22c55e]" : "bg-[#ef4444]"}`} />
              <div>
                <div className="font-medium">{s.name}</div>
                <div className="text-xs text-muted">{s.path}</div>
              </div>
              {s.name === current.name && <span className="ml-auto text-sm text-[#22c55e] font-bold">✓</span>}
            </Link>
          ))}
          <div className="flex gap-2 mt-3 pt-3 border-t border-line">
            <Link to="/sites" className="text-xs px-3 py-1.5 rounded border border-line hover:bg-surface" onClick={() => setOpen(false)}>Manage Sites</Link>
            <Link to="/sites/new" className="text-xs px-3 py-1.5 rounded border border-line hover:bg-surface" onClick={() => setOpen(false)}>+ Add New Site</Link>
          </div>
        </div>
      )}
    </div>
  );
}
