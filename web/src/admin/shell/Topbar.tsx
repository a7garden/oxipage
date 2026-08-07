import { useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { SiteSelector } from "./SiteSelector";
import { listSites } from "../shared/api";
import { Settings } from "lucide-react";
import { ThemeToggle } from "../../shared/ThemeToggle";


function SiteContextInfo() {
  const { slug } = useParams();
  const { data } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const site = data?.data?.find((s) => slug ? s.name === slug : s.active);
  if (!site) return null;
  return (
    <div>
      <div className="text-sm font-medium">{site.name}</div>
      <div className="text-xs text-muted">{site.path}</div>
    </div>
  );
}

/**
 * Console topbar — sticky band anchored to the top of <ConsoleShell>.
 *
 * Tone: backdrop is the canvas color so the bar blends with the page on
 * scroll; the bottom border is a single hairline border-line. The brand
 * logo uses text-primary (the semantic accent token), which auto-switches
 * between --p-accent-600 (light) and --p-accent-400 (dark) without inline
 * style hacks.
 */
export function Topbar() {
  return (
    <header className="sticky top-0 z-40 h-12 flex items-center px-4 gap-2 bg-canvas border-b border-line">
      <div className="font-display text-[15px] font-bold shrink-0 text-primary">
        oxibuilder
      </div>
      <SiteSelector />
      <div className="w-px h-6 bg-line mx-2" />
      <SiteContextInfo />
      <div className="ml-auto flex items-center gap-1">
        <ThemeToggle />
        <button
          className="inline-flex items-center justify-center size-8 rounded-md text-muted hover:text-foreground hover:bg-surface/50 transition-colors shrink-0"
          aria-label="Settings"
        >
          <Settings size={16} />
        </button>
      </div>
    </header>
  );
}
