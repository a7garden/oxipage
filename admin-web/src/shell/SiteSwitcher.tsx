import { useRef, useState, useEffect } from "react";
import { ChevronDown } from "lucide-react";
import { useSite } from "../shared/api";

export function SiteSwitcher() {
  const { activeSite, sites, setActiveSite } = useSite();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  if (sites.length === 0) {
    return <span className="text-xs text-muted">No sites configured</span>;
  }

  return (
    <div ref={ref} className="relative">
      <button
        className="site-switcher-trigger"
        onClick={() => setOpen(!open)}
      >
        <span
          aria-hidden="true"
          className={`inline-block h-2 w-2 rounded-full ${
            activeSite ? "bg-positive" : "bg-raised"
          }`}
        />
        {activeSite?.name || "Select site"}
        <ChevronDown size={14} />
      </button>

      {open && (
        <div className="absolute left-0 top-full z-50 mt-1 min-w-[220px] overflow-hidden rounded-md border border-line bg-surface shadow-md">
          {sites.map((site) => (
            <button
              key={site.name}
              className={`block w-full cursor-pointer border-none px-3 py-2 text-left text-sm ${
                site.name === activeSite?.name ? "bg-raised" : "bg-transparent"
              }`}
              onClick={() => {
                setActiveSite(site.name);
                setOpen(false);
              }}
            >
              <strong>{site.name}</strong>
              <span className="mt-0.5 block text-[11px] text-muted">
                {site.endpoint}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
