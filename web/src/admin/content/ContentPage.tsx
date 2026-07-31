import { useState } from "react";
import { useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { cn } from "../../shared/ui/cn";
import { listExtensions } from "../shared/api";
import { BlogTab } from "./BlogTab";
import { ProjectsTab } from "./ProjectsTab";
import { LinksTab } from "./LinksTab";
import { MoviesTab } from "./MoviesTab";
import { BooksTab } from "./BooksTab";
import { NovelsTab } from "./NovelsTab";
import { ScrapsTab } from "./ScrapsTab";
import { ProfileTab } from "./ProfileTab";

const tabs = [
  { id: "profile", label: "Profile" },
  { id: "blog", label: "Blog" },
  { id: "projects", label: "Projects" },
  { id: "links", label: "Links" },
  { id: "movies", label: "Movies" },
  { id: "books", label: "Books" },
  { id: "novels", label: "Novels" },
  { id: "scraps", label: "Scraps" },
] as const;

const tabComponents: Record<string, React.FC<{ slug: string }>> = {
  profile: ProfileTab,
  blog: BlogTab,
  projects: ProjectsTab,
  links: LinksTab,
  movies: MoviesTab,
  books: BooksTab,
  novels: NovelsTab,
  scraps: ScrapsTab,
};

export function ContentPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const [activeTab, setActiveTab] = useState("profile");
  const { data: extensions = [] } = useQuery({
    queryKey: ["site", slug, "extensions"],
    queryFn: () => listExtensions(slug!),
    enabled: !!slug,
  });
  const enabledIds = new Set(
    extensions.filter((e) => e.enabled).map((e) => e.id),
  );
  const visibleTabs = tabs.filter(
    (t) => t.id === "profile" || enabledIds.has(t.id),
  );
  const active = visibleTabs.some((t) => t.id === activeTab) ? activeTab : "profile";
  const TabComponent = tabComponents[active];

  return (
    <div>
      <h1 className="text-xl font-bold text-foreground mb-1">Content</h1>
      <p className="text-sm text-muted mb-4">Manage all content across extensions</p>

      <div className="flex gap-0 border-b-2 border-line mb-4">
        {visibleTabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={cn(
              "px-4 py-2 text-sm font-medium border-b-2 -mb-[2px] transition-colors",
              active === tab.id
                ? "text-[#2a6b4a] border-[#22c55e]"
                : "text-muted border-transparent hover:text-foreground",
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {slug && <TabComponent slug={slug} />}
    </div>
  );
}
