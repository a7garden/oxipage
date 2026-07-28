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
    return <span className="text-xs text-[#777]">No sites configured</span>;
  }

  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button
        className="site-switcher-trigger"
        onClick={() => setOpen(!open)}
      >
        <span
          style={{
            display: "inline-block",
            width: 8,
            height: 8,
            borderRadius: "50%",
            background: activeSite ? "oklch(60% 0.15 145)" : "#ccc",
          }}
        />
        {activeSite?.name || "Select site"}
        <ChevronDown size={14} />
      </button>

      {open && (
        <div
          style={{
            position: "absolute",
            top: "100%",
            left: 0,
            marginTop: 4,
            minWidth: 220,
            background: "#fff",
            border: "1px solid #e8e4e0",
            borderRadius: 6,
            boxShadow: "0 4px 12px rgba(0,0,0,0.08)",
            zIndex: 50,
            overflow: "hidden",
          }}
        >
          {sites.map((site) => (
            <button
              key={site.name}
              style={{
                display: "block",
                width: "100%",
                padding: "8px 12px",
                textAlign: "left",
                fontSize: 14,
                border: "none",
                background: site.name === activeSite?.name ? "#f5f2ed" : "transparent",
                cursor: "pointer",
              }}
              onClick={() => {
                setActiveSite(site.name);
                setOpen(false);
              }}
            >
              <strong>{site.name}</strong>
              <span style={{ display: "block", fontSize: 11, color: "#888", marginTop: 1 }}>
                {site.endpoint}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
