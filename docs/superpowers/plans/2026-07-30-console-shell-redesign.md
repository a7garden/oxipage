# Console Shell Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current header-only SiteShell with a GCP-style site-scoped console: dark sidebar nav, site selector dropdown, 6 functional pages (Dashboard, Content, Extensions, Themes, Deploy, Settings).

**Architecture:** Single Vite project (`web/src/admin/`). ConsoleShell wraps all routes with topbar + sidebar. Each page is a separate component under `web/src/admin/{page}/`. All API calls go through `siteScopedFetch(slug, path)` to `/api/console/s/{slug}*`. Existing extension REST APIs are reused for content tabs.

**Tech Stack:** React 19, React Router v7, TanStack Query v5, Tailwind v4, lucide-react, shared UI components from `web/src/shared/ui/`.

## Global Constraints

- All new code goes into `web/src/admin/` — no separate `admin-web/` project
- Console sidebar background: `#1a1e24` (dark, not affected by `data-theme` toggle). Main content respects light/dark toggle.
- Active sidebar item: green left border (`#22c55e`), green text (`#4ade80`), green tint bg (`rgba(34, 197, 94, 0.1)`)
- Every page handles loading (Skeleton), empty (EmptyState), and error (inline error + retry) states
- Existing UI components from `web/src/shared/ui/` must be reused (Button, Card, Badge, EmptyState, Skeleton, Tabs, DropdownMenu, Input, Container, cn)
- SVG icons from lucide-react (already in package.json)
- All API calls use existing `siteScopedFetch(slug, path)` from `web/src/admin/shared/api.ts`
- `web/src/admin/shell/SiteShell.tsx` is replaced by ConsoleShell, NOT deleted until ConsoleShell is verified working

---
## Task 1: ConsoleShell + Routing Restructure

**Files:**
- Create: `web/src/admin/shell/ConsoleShell.tsx`
- Create: `web/src/admin/shell/Sidebar.tsx`
- Create: `web/src/admin/shell/Topbar.tsx`
- Create: `web/src/admin/shell/SiteSelector.tsx`
- Modify: `web/src/admin/App.tsx` (routing restructure)
- Modify: `web/src/admin/shared/api.ts` (add site-fetch helpers)

**Interfaces:**
- Consumes: `listSites()`, `SiteInfo`, `getDefaultSite()` from `shared/api.ts`
- Produces: `ConsoleShell` (wraps `<Topbar /> + <Sidebar /> + <Outlet />`), `Sidebar` (nav items + active state), `Topbar` (logo + SiteSelector + actions), `SiteSelector` (dropdown panel)

- [ ] **Step 1: Create ConsoleShell.tsx**

```tsx
import { Outlet } from "react-router";
import { Topbar } from "./Topbar";
import { Sidebar } from "./Sidebar";

export function ConsoleShell() {
  return (
    <div className="min-h-screen bg-canvas flex flex-col">
      <Topbar />
      <div className="flex flex-1">
        <Sidebar />
        <main className="flex-1 p-6 overflow-auto">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
```

Layout: topbar fixed height 48px. Below it: sidebar 200px (flex-shrink-0) + main flex-1.

- [ ] **Step 2: Create Topbar.tsx**

```tsx
import { SiteSelector } from "./SiteSelector";

export function Topbar() {
  return (
    <header className="sticky top-0 z-40 h-12 flex items-center px-4 gap-2 bg-canvas border-b border-line">
      <div className="font-display text-[15px] font-bold text-[#2a6b4a] shrink-0">
        oxibuilder
      </div>
      <SiteSelector />
      <div className="w-px h-6 bg-line mx-2" />
      <SiteContextInfo />
      <div className="ml-auto flex items-center gap-1">
        <ThemeToggle />
        <button className="icon-btn" aria-label="Settings">⚙</button>
      </div>
    </header>
  );
}
```

- `ThemeToggle` — same logic as existing SiteShell's ThemeToggle (dark state + localStorage + data-theme attribute)
- `SiteContextInfo` — reads current `slug` from `useParams()`, shows site title from sites list
- Icons: use lucide-react (`Sun`, `Moon`, `Settings`) instead of text chars

- [ ] **Step 3: Create SiteSelector.tsx**

Site selector is a dropdown that shows current site with green indicator. On click, shows a panel listing all sites with status dots + name + URL. Active site has ✓ checkmark. Bottom has "Manage Sites" and "Add New Site" links.

```tsx
import { useState, useRef, useEffect } from "react";
import { useParams, Link } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { listSites } from "../shared/api";
import { cn } from "../../shared/ui/cn";

export function SiteSelector() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const { slug } = useParams();
  const { data } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const sites = data?.data ?? [];

  // Close on click outside
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  const current = sites.find((s) => slug ? s.name === slug : s.active) ?? sites[0];

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 px-2.5 py-1 border border-line rounded-md text-sm font-medium bg-surface/50 hover:bg-surface min-w-[140px]"
      >
        <span className="size-2 rounded-full bg-[#22c55e] shrink-0" />
        <span>{current?.name ?? "Select site"}</span>
        <span className="ml-auto text-[10px] text-muted">▾</span>
      </button>

      {open && (
        <div className="absolute top-full left-0 mt-1 w-[420px] bg-canvas border border-line rounded-lg shadow-xl z-50 p-3">
          <div className="flex gap-4">
            <div className="flex-1">
              <div className="text-[11px] font-semibold uppercase tracking-wide text-muted mb-2">
                Your Sites
              </div>
              {sites.map((s) => (
                <Link
                  key={s.name}
                  to={`/s/${s.name}`}
                  onClick={() => setOpen(false)}
                  className={cn(
                    "flex items-center gap-2.5 px-3 py-2 rounded-md text-sm",
                    s.name === current?.name
                      ? "bg-[rgba(34,197,94,0.08)]"
                      : "hover:bg-surface"
                  )}
                >
                  <span className={cn(
                    "size-2 rounded-full shrink-0",
                    s.active ? "bg-[#22c55e]" : "bg-[#ef4444]"
                  )} />
                  <div>
                    <div className="font-medium">{s.name}</div>
                    <div className="text-xs text-muted">{s.path}</div>
                  </div>
                  {s.name === current?.name && (
                    <span className="ml-auto text-sm text-[#22c55e] font-bold">✓</span>
                  )}
                </Link>
              ))}
            </div>
          </div>
          <div className="flex gap-2 mt-3 pt-3 border-t border-line">
            <Link to="/sites" className="text-xs px-3 py-1.5 rounded border border-line hover:bg-surface">
              Manage Sites
            </Link>
            <Link to="/sites/new" className="text-xs px-3 py-1.5 rounded border border-line hover:bg-surface">
              + Add New Site
            </Link>
          </div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Create Sidebar.tsx**

```tsx
import { NavLink } from "react-router";
import { useParams } from "react-router";
import { cn } from "../../shared/ui/cn";

const navItems = [
  { section: "General", items: [
    { icon: "◉", label: "Dashboard", path: "" },
    { icon: "☰", label: "Content", path: "content" },
    { icon: "◆", label: "Extensions", path: "extensions" },
  ]},
  { section: "Appearance", items: [
    { icon: "◐", label: "Themes", path: "themes" },
  ]},
  { section: "Operations", items: [
    { icon: "↻", label: "Deploy", path: "deploy" },
    { icon: "⚙", label: "Settings", path: "settings" },
  ]},
];

export function Sidebar() {
  const { slug } = useParams();

  return (
    <aside className="w-[200px] shrink-0 flex flex-col" style={{ backgroundColor: "#1a1e24" }}>
      <nav className="flex-1 pt-2">
        {navItems.map((group) => (
          <div key={group.section}>
            <div className="px-4 pt-4 pb-1.5 text-[10px] font-semibold uppercase tracking-wider" style={{ color: "#6b7280" }}>
              {group.section}
            </div>
            {group.items.map((item) => {
              const to = item.path ? `/s/${slug}/${item.path}` : `/s/${slug}`;
              return (
                <NavLink
                  key={item.label}
                  to={to}
                  end={item.path === ""}
                  className={({ isActive }) => cn(
                    "flex items-center gap-2.5 px-4 py-2 text-sm border-l-[3px] border-transparent transition-all",
                    isActive
                      ? "text-[#4ade80] border-l-[#22c55e]"
                      : "text-[#9ca3af] hover:text-[#e5e7eb] hover:bg-[rgba(255,255,255,0.04)]"
                  )}
                  style={({ isActive }) => isActive ? { backgroundColor: "rgba(34, 197, 94, 0.1)" } : {}}
                >
                  <span className="text-sm opacity-60">{item.icon}</span>
                  {item.label}
                </NavLink>
              );
            })}
          </div>
        ))}
      </nav>
      <div className="px-4 py-3 border-t border-[rgba(255,255,255,0.06)] text-xs" style={{ color: "#6b7280" }}>
        v1.0.0 · {slug ?? "no site"}
      </div>
    </aside>
  );
}
```

- [ ] **Step 5: Restructure App.tsx routing**

Current: one `DashboardPage` at `/s/:slug`. New: 6 child routes under `/s/:slug`.

```tsx
import { lazy, Suspense } from "react";
import { BrowserRouter, Routes, Route } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ConsoleShell } from "./shell/ConsoleShell";
import { HomeRedirect } from "./sites/HomeRedirect";
import { SitesPage } from "./sites/SitesPage";
import { NewSiteWizardPage } from "./sites/NewSiteWizardPage";
import { SetupGuard } from "../setup/SetupGuard";
import { Skeleton } from "../shared/ui/skeleton";

// Lazy-loaded pages
const DashboardPage = lazy(() => import("./dashboard/DashboardPage").then(m => ({ default: m.DashboardPage })));
const ContentPage = lazy(() => import("./content/ContentPage").then(m => ({ default: m.ContentPage })));
const ExtensionsPage = lazy(() => import("./extensions/ExtensionsPage").then(m => ({ default: m.ExtensionsPage })));
const ThemesPage = lazy(() => import("./themes/ThemesPage").then(m => ({ default: m.ThemesPage })));
const DeployPage = lazy(() => import("./deploy/DeployPage").then(m => ({ default: m.DeployPage })));
const SettingsPage = lazy(() => import("./settings/SettingsPage").then(m => ({ default: m.SettingsPage })));
const SetupWizard = lazy(() => import("../setup/SetupWizard").then(m => ({ default: m.SetupWizard })));

const queryClient = new QueryClient();

function ShellFallback() {
  return (
    <div className="flex">
      <aside className="w-[200px]" style={{ backgroundColor: "#1a1e24", minHeight: "100vh" }} />
      <div className="flex-1 p-6 space-y-4">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-32 w-full" />
        <Skeleton className="h-32 w-full" />
      </div>
    </div>
  );
}

export function AdminApp() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route path="/setup/*" element={<SetupGuard><Suspense fallback={<div className="min-h-screen flex items-center justify-center"><div className="animate-pulse text-subtle">Loading...</div></div>}><SetupWizard /></Suspense></SetupGuard>} />
          <Route element={<ConsoleShell />}>
            <Route index element={<SetupGuard fullPage={false}><HomeRedirect /></SetupGuard>} />
            <Route path="sites" element={<SetupGuard fullPage={false}><SitesPage /></SetupGuard>} />
            <Route path="sites/new" element={<NewSiteWizardPage />} />
            <Route path="s/:slug" element={<Suspense fallback={<ShellFallback />}><DashboardPage /></Suspense>} />
            <Route path="s/:slug/content" element={<Suspense fallback={<ShellFallback />}><ContentPage /></Suspense>} />
            <Route path="s/:slug/extensions" element={<Suspense fallback={<ShellFallback />}><ExtensionsPage /></Suspense>} />
            <Route path="s/:slug/themes" element={<Suspense fallback={<ShellFallback />}><ThemesPage /></Suspense>} />
            <Route path="s/:slug/deploy" element={<Suspense fallback={<ShellFallback />}><DeployPage /></Suspense>} />
            <Route path="s/:slug/settings" element={<Suspense fallback={<ShellFallback />}><SettingsPage /></Suspense>} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
```

- [ ] **Step 6: Verify shell renders**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors. Shell appears with dark sidebar + topbar + routes working.

- [ ] **Step 7: Commit shell foundation**

```bash
git add web/src/admin/shell/ web/src/admin/App.tsx
git commit -m "feat(admin): ConsoleShell with dark sidebar, site selector, restructured routing"
```

---
## Task 2: DashboardPage

**Files:**
- Create: `web/src/admin/shared/stat-card.tsx`
- Create: `web/src/admin/dashboard/DashboardPage.tsx`

**Interfaces:**
- Consumes: `useParams().slug`, `siteScopedFetch(slug, "/stats")`, `siteScopedFetch(slug, "/blog/posts?limit=5")`, `listSites()`
- Produces: `DashboardPage` with 4 stat cards, recent posts table, quick actions

- [ ] **Step 1: Create stat-card.tsx**

```tsx
interface StatCardProps {
  label: string;
  value: string | number;
  change?: string;
  changeColor?: string;
}

export function StatCard({ label, value, change, changeColor }: StatCardProps) {
  return (
    <div className="border border-line rounded-lg p-4 bg-surface/30">
      <div className="text-xs text-muted mb-1">{label}</div>
      <div className="text-2xl font-bold text-foreground">{value}</div>
      {change && (
        <div className="text-xs mt-0.5" style={{ color: changeColor ?? "#22c55e" }}>
          {change}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create DashboardPage.tsx**

```tsx
import { useParams, useNavigate } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { listSites, siteScopedFetch } from "../shared/api";
import { StatCard } from "../shared/stat-card";
import { Button } from "../../shared/ui/button";
import { Badge } from "../../shared/ui/badge";
import { Skeleton } from "../../shared/ui/skeleton";
import { EmptyState, EmptyStateIcon, EmptyStateTitle, EmptyStateDescription } from "../../shared/ui/empty-state";

export function DashboardPage() {
  const { slug } = useParams();
  const navigate = useNavigate();

  const { data: sitesData } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const siteName = sitesData?.data?.find((s) => s.name === slug)?.name ?? slug;

  // Stats — v1: use static numbers as placeholder, v2: real API
  const { data: recent, isLoading } = useQuery({
    queryKey: ["site", slug, "recent"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug!, "/blog/posts?limit=5");
      if (!res.ok) return [];
      const json = await res.json();
      return json.data ?? [];
    },
    enabled: !!slug,
  });

  return (
    <div>
      {/* Page header */}
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Dashboard</h1>
          <p className="text-sm text-muted mt-0.5">{siteName} · Last deployed 2h ago</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => fetch(`/api/console/s/${slug}/build`, { method: "POST" })}>
            ↻ Rebuild
          </Button>
          <Button onClick={() => fetch(`/api/console/s/${slug}/deploy`, { method: "POST" })}>
            ⇧ Deploy
          </Button>
        </div>
      </div>

      {/* Stats cards */}
      <div className="grid grid-cols-4 gap-3 mb-6">
        <StatCard label="Extensions" value={8} change="all active" />
        <StatCard label="Posts" value="—" change="coming soon" />
        <StatCard label="Storage" value="—" change="coming soon" />
        <StatCard label="Uptime" value="—" change="server online" />
      </div>

      {/* Recent posts */}
      <h2 className="text-sm font-semibold text-foreground mb-3">Recent Posts</h2>
      {isLoading ? (
        <div className="space-y-2">{[1,2,3].map(i => <Skeleton key={i} className="h-12 w-full" />)}</div>
      ) : recent && recent.length > 0 ? (
        <div className="border border-line rounded-lg overflow-hidden">
          <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider">
            <div className="flex-1 px-4 py-2.5">Title</div>
            <div className="w-20 px-4 py-2.5">Status</div>
            <div className="w-28 px-4 py-2.5">Updated</div>
          </div>
          {recent.map((post: any) => (
            <div key={post.id ?? post.slug} className="flex border-t border-line text-sm hover:bg-surface/30 cursor-pointer">
              <div className="flex-1 px-4 py-2.5 truncate">{post.title}</div>
              <div className="w-20 px-4 py-2.5">
                <Badge variant={post.published_at ? "success" : "warning"}>
                  {post.published_at ? "Published" : "Draft"}
                </Badge>
              </div>
              <div className="w-28 px-4 py-2.5 text-muted text-xs">
                {post.published_at ?? post.updated_at ?? "—"}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <EmptyState>
          <EmptyStateTitle>No posts yet</EmptyStateTitle>
          <EmptyStateDescription>Write your first post in the Content section.</EmptyStateDescription>
        </EmptyState>
      )}

      {/* Quick actions */}
      <h2 className="text-sm font-semibold text-foreground mt-6 mb-3">Quick Actions</h2>
      <div className="flex gap-2">
        <Button variant="outline" onClick={() => navigate(`/s/${slug}/content`)}>✏ New Post</Button>
        <Button variant="outline" onClick={() => navigate(`/s/${slug}/extensions`)}>📦 Install Extension</Button>
        <Button variant="outline" onClick={() => navigate(`/s/${slug}/deploy`)}>⚡ Build & Deploy</Button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: TypeScript check**

Run: `cd web && npx tsc --noEmit`
Expected: Clean

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/dashboard/ web/src/admin/shared/stat-card.tsx
git commit -m "feat(admin): DashboardPage with stat cards, recent posts, quick actions"
```

---
## Task 3: ContentPage with Tabs

**Files:**
- Create: `web/src/admin/content/ContentPage.tsx`
- Create: `web/src/admin/content/BlogTab.tsx`
- Create: `web/src/admin/content/ProjectsTab.tsx`
- Create: `web/src/admin/content/LinksTab.tsx`
- Create: `web/src/admin/content/MoviesTab.tsx`
- Create: `web/src/admin/content/BooksTab.tsx`
- Create: `web/src/admin/content/NovelsTab.tsx`
- Create: `web/src/admin/content/ScrapsTab.tsx`
- Create: `web/src/admin/shared/content-table.tsx`

**Interfaces:**
- Consumes: `useParams().slug`, `siteScopedFetch(slug, "/{ext}/{resource}")` per extension
- Produces: `ContentPage` (tab container), per-extension tab components

- [ ] **Step 1: Create content-table.tsx**

```tsx
import type { ReactNode } from "react";
import { EmptyState, EmptyStateTitle, EmptyStateDescription } from "../../shared/ui/empty-state";

interface Column {
  key: string;
  label: string;
  width?: string;
  render?: (row: any) => ReactNode;
}

interface ContentTableProps {
  columns: Column[];
  data: any[];
  isLoading: boolean;
  emptyTitle?: string;
  emptyDescription?: string;
  onRowClick?: (row: any) => void;
}

export function ContentTable({ columns, data, isLoading, emptyTitle, emptyDescription, onRowClick }: ContentTableProps) {
  if (isLoading) {
    return <div className="space-y-2">{[1,2,3].map(i => <div key={i} className="h-12 bg-surface/50 rounded animate-pulse" />)}</div>;
  }

  if (data.length === 0) {
    return (
      <EmptyState>
        <EmptyStateTitle>{emptyTitle ?? "No content"}</EmptyStateTitle>
        <EmptyStateDescription>{emptyDescription ?? "Create your first item."}</EmptyStateDescription>
      </EmptyState>
    );
  }

  return (
    <div className="border border-line rounded-lg overflow-hidden">
      <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider border-b border-line">
        {columns.map((col) => (
          <div key={col.key} className="px-4 py-2.5" style={{ flex: col.width ? `0 0 ${col.width}` : 1 }}>
            {col.label}
          </div>
        ))}
      </div>
      {data.map((row, i) => (
        <div
          key={row.id ?? row.slug ?? i}
          className="flex border-t border-line text-sm hover:bg-surface/30 cursor-pointer"
          onClick={() => onRowClick?.(row)}
        >
          {columns.map((col) => (
            <div key={col.key} className="px-4 py-2.5 truncate" style={{ flex: col.width ? `0 0 ${col.width}` : 1 }}>
              {col.render ? col.render(row) : row[col.key] ?? "—"}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Create ContentPage.tsx**

```tsx
import { useState } from "react";
import { useParams } from "react-router";
import { cn } from "../../shared/ui/cn";
import { BlogTab } from "./BlogTab";
import { ProjectsTab } from "./ProjectsTab";
import { LinksTab } from "./LinksTab";
import { MoviesTab } from "./MoviesTab";
import { BooksTab } from "./BooksTab";
import { NovelsTab } from "./NovelsTab";
import { ScrapsTab } from "./ScrapsTab";

const tabs = [
  { id: "blog", label: "Blog" },
  { id: "projects", label: "Projects" },
  { id: "links", label: "Links" },
  { id: "movies", label: "Movies" },
  { id: "books", label: "Books" },
  { id: "novels", label: "Novels" },
  { id: "scraps", label: "Scraps" },
];

const tabComponents: Record<string, React.FC<{ slug: string }>> = {
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
  const [activeTab, setActiveTab] = useState("blog");
  const ActiveComponent = tabComponents[activeTab];

  return (
    <div>
      <h1 className="text-xl font-bold text-foreground mb-1">Content</h1>
      <p className="text-sm text-muted mb-4">Manage all content across extensions</p>

      {/* Tabs */}
      <div className="flex gap-0 border-b-2 border-line mb-4">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={cn(
              "px-4 py-2 text-sm font-medium border-b-2 -mb-[2px] transition-colors",
              activeTab === tab.id
                ? "text-[#2a6b4a] border-[#22c55e]"
                : "text-muted border-transparent hover:text-foreground"
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Active tab content */}
      {slug && <ActiveComponent slug={slug} />}
    </div>
  );
}
```

- [ ] **Step 3-9: Create each extension tab** (BlogTab, ProjectsTab, LinksTab, MoviesTab, BooksTab, NovelsTab, ScrapsTab)

Each tab follows the same pattern:

```tsx
// BlogTab.tsx example
import { useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Badge } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";

export function BlogTab({ slug }: { slug: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "blog"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug, "/blog/posts");
      if (!res.ok) return [];
      return (await res.json()).data ?? [];
    },
  });

  const columns = [
    { key: "title", label: "Title" },
    { key: "status", label: "Status", width: "80px", render: (row: any) => (
      <Badge variant={row.published_at ? "success" : "warning"}>
        {row.published_at ? "Published" : "Draft"}
      </Badge>
    )},
    { key: "lang", label: "Lang", width: "60px", render: (row: any) => (
      <span className="text-muted text-xs">{row.lang ?? "—"}</span>
    )},
    { key: "updated", label: "Updated", width: "100px", render: (row: any) => (
      <span className="text-muted text-xs">{row.updated_at ?? row.published_at ?? "—"}</span>
    )},
  ];

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <input placeholder="Search posts..." className="w-60 px-3 py-1.5 border border-line rounded-md text-sm bg-surface/50" />
        <Button>+ New Post</Button>
      </div>
      <ContentTable columns={columns} data={data ?? []} isLoading={isLoading}
        emptyTitle="No posts yet" emptyDescription="Write your first blog post."
      />
    </div>
  );
}
```

Each tab uses its extension's API endpoint (`/blog/posts`, `/projects`, `/links`, `/movies`, `/books`, `/novels`, `/scraps`) via `siteScopedFetch`. Columns per spec §4.2 table.

- [ ] **Step 10: TypeScript check**

Run: `cd web && npx tsc --noEmit`
Expected: Clean

- [ ] **Step 11: Commit**

```bash
git add web/src/admin/content/ web/src/admin/shared/content-table.tsx
git commit -m "feat(admin): ContentPage with per-extension tabs (Blog/Projects/Links/Movies/Books/Novels/Scraps)"
```

---
## Task 4: ExtensionsPage + ThemesPage

**Files:**
- Create: `web/src/admin/extensions/ExtensionsPage.tsx`
- Create: `web/src/admin/themes/ThemesPage.tsx`

**Interfaces:**
- Consumes: `useParams().slug`, `siteScopedFetch(slug, "/extensions")`, `PUT .../extensions/{id}/disable`, `siteScopedFetch(slug, "/theme")`
- Produces: `ExtensionsPage` (installed grid + registry), `ThemesPage` (catalog + preview)

- [ ] **Step 1: Create ExtensionsPage.tsx**

```tsx
import { useParams } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { siteScopedFetch } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Skeleton } from "../../shared/ui/skeleton";
import { cn } from "../../shared/ui/cn";

export function ExtensionsPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const qc = useQueryClient();
  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "extensions"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug, "/extensions");
      if (!res.ok) return { installed: [], available: [] };
      return (await res.json()).data ?? { installed: [], available: [] };
    },
  });

  const toggleExt = useMutation({
    mutationFn: async ({ id, enable }: { id: string; enable: boolean }) => {
      const method = enable ? "PUT" : "DELETE";
      await siteScopedFetch(slug!, `/extensions/${id}${enable ? "/enable" : ""}`);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "extensions"] }),
  });

  const installed = data?.installed ?? [];
  const available = data?.available ?? [];
  const allExtensions = [...installed, ...available];

  return (
    <div>
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Extensions</h1>
          <p className="text-sm text-muted mt-0.5">Manage installed extensions for {slug}</p>
        </div>
        <Button>+ Install Extension</Button>
      </div>

      {isLoading ? (
        <div className="grid grid-cols-2 gap-3">
          {[1,2,3,4].map(i => <Skeleton key={i} className="h-16" />)}
        </div>
      ) : allExtensions.length === 0 ? (
        <div className="text-center py-12 text-muted">No extensions available.</div>
      ) : (
        <>
          {installed.length > 0 && (
            <>
              <div className="text-xs font-semibold uppercase tracking-wider text-muted mb-3">
                Installed ({installed.length})
              </div>
              <div className="grid grid-cols-2 gap-3 mb-6">
                {installed.map((ext: any) => (
                  <div key={ext.id} className={cn(
                    "border border-line rounded-lg p-3 flex items-center gap-3",
                    !ext.enabled && "opacity-50"
                  )}>
                    <div className="size-9 rounded-lg bg-[#dcfce7] text-[#166534] flex items-center justify-center text-base font-bold shrink-0">
                      {ext.id?.[0]?.toUpperCase() ?? "?"}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium">{ext.name ?? ext.id}</div>
                      <div className="text-xs text-muted truncate">{ext.crate_id}</div>
                    </div>
                    <Button
                      variant={ext.enabled ? "destructive" : "outline"}
                      size="sm"
                      onClick={() => toggleExt.mutate({ id: ext.id, enable: !ext.enabled })}
                    >
                      {ext.enabled ? "Disable" : "Enable"}
                    </Button>
                  </div>
                ))}
              </div>
            </>
          )}

          {available.length > 0 && (
            <>
              <div className="text-xs font-semibold uppercase tracking-wider text-muted mb-3">
                Available from Registry
              </div>
              <div className="grid grid-cols-2 gap-3">
                {available.map((ext: any) => (
                  <div key={ext.id} className="border border-line rounded-lg p-3 flex items-center gap-3 opacity-50">
                    <div className="size-9 rounded-lg bg-[#f0f0ee] text-[#aaa] flex items-center justify-center text-base font-bold shrink-0">
                      {ext.id?.[0]?.toUpperCase() ?? "?"}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium">{ext.name ?? ext.id}</div>
                      <div className="text-xs text-muted truncate">{ext.crate_id}</div>
                    </div>
                    <Button variant="outline" size="sm">Install</Button>
                  </div>
                ))}
              </div>
            </>
          )}
        </>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create ThemesPage.tsx**

```tsx
import { useState } from "react";
import { useParams } from "react-router";
import { cn } from "../../shared/ui/cn";

const themes = [
  { id: "paper", name: "Paper", light: "oklch(98.5% 0.004 95)", dark: "oklch(13% 0.020 265)", accent: "#22c55e", body: "oklch(75% 0.010 95)" },
  { id: "midnight", name: "Midnight", light: "oklch(10% 0.025 265)", dark: "oklch(96% 0.005 250)", accent: "#4ade80", body: "oklch(35% 0.012 265)" },
  { id: "sepia", name: "Sepia", light: "oklch(96% 0.02 80)", dark: "oklch(15% 0.015 60)", accent: "#eab308", body: "oklch(82% 0.15 85)" },
  { id: "forest", name: "Forest", light: "oklch(97% 0.01 145)", dark: "oklch(12% 0.02 155)", accent: "#22c55e", body: "oklch(75% 0.010 145)" },
];

function ThemePreview({ theme }: { theme: typeof themes[0] }) {
  return (
    <div className="h-20 p-3 rounded-t-lg" style={{ background: theme.light }}>
      <div className="text-xs font-bold mb-1.5" style={{ color: theme.dark }}>{theme.name}</div>
      <div className="h-1.5 rounded-sm mb-1" style={{ width: "70%", background: theme.accent }} />
      <div className="h-1.5 rounded-sm mb-1" style={{ width: "50%", background: theme.body }} />
      <div className="h-1.5 rounded-sm" style={{ width: "30%", background: theme.body }} />
    </div>
  );
}

export function ThemesPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const [current, setCurrent] = useState("paper");

  return (
    <div>
      <h1 className="text-xl font-bold text-foreground mb-1">Themes</h1>
      <p className="text-sm text-muted mb-6">Pick a visual theme for the public site</p>

      <div className="grid grid-cols-4 gap-3 mb-6">
        {themes.map((theme) => (
          <button
            key={theme.id}
            onClick={() => setCurrent(theme.id)}
            className={cn(
              "border rounded-lg overflow-hidden text-left cursor-pointer transition-all",
              current === theme.id ? "border-[#22c55e] border-2" : "border-line hover:border-[#22c55e]"
            )}
          >
            <ThemePreview theme={theme} />
            <div className="px-3 py-2 border-t border-line flex items-center justify-between">
              <span className="text-sm font-medium">{theme.name}</span>
              {current === theme.id && <span className="text-xs font-bold text-[#22c55e]">✓ Current</span>}
            </div>
          </button>
        ))}
      </div>

      {/* Live preview */}
      <div className="border border-line rounded-lg p-5 bg-surface/30">
        <div className="text-xs font-semibold text-muted uppercase tracking-wider mb-3">
          Preview — {slug} landing page
        </div>
        <div className="flex items-center justify-between mb-4">
          <div className="font-display text-lg font-bold">My Blog</div>
          <div className="text-xs text-muted">🌙 · ko</div>
        </div>
        <div className="grid grid-cols-3 gap-3">
          {[
            { title: "Console Redesign", excerpt: "A new era for Oxibuilder management..." },
            { title: "WASM v2 Benchmarks", excerpt: "Performance numbers for the new runtime..." },
            { title: "멀티사이트 가이드", excerpt: "여러 블로그를 한 번에 관리하는 방법..." },
          ].map((item) => (
            <div key={item.title} className="border border-line rounded-md p-3 bg-canvas">
              <div className="text-sm font-semibold mb-1">{item.title}</div>
              <div className="text-xs text-muted">{item.excerpt}</div>
            </div>
          ))}
        </div>
      </div>

      <div className="flex justify-end mt-4">
        <Button onClick={() => fetch(`/api/console/s/${slug}/theme`, {
          method: "PUT",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ theme_id: current }),
        })}>
          Apply Theme
        </Button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: TypeScript check**

Run: `cd web && npx tsc --noEmit`
Expected: Clean

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/extensions/ web/src/admin/themes/
git commit -m "feat(admin): ExtensionsPage (grid + toggle) and ThemesPage (catalog + preview)"
```

---
## Task 5: DeployPage + SettingsPage

**Files:**
- Create: `web/src/admin/deploy/DeployPage.tsx`
- Create: `web/src/admin/settings/SettingsPage.tsx`
- Modify: `web/src/admin/shared/api.ts` (add config endpoints)

- [ ] **Step 1: Create DeployPage.tsx**

```tsx
import { useState } from "react";
import { useParams } from "react-router";
import { useQuery, useMutation } from "@tanstack/react-query";
import { siteScopedFetch, triggerBuild } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Badge } from "../../shared/ui/badge";
import { Skeleton } from "../../shared/ui/skeleton";

export function DeployPage() {
  const { slug } = useParams<{ slug: string }>()!;
  const [buildLog, setBuildLog] = useState<string[]>([]);

  const { data: builds, isLoading } = useQuery({
    queryKey: ["site", slug, "builds"],
    queryFn: async () => {
      const res = await siteScopedFetch(slug!, "/builds");
      if (!res.ok) return [];
      return (await res.json()).data ?? [];
    },
    enabled: !!slug,
  });

  const doBuild = useMutation({
    mutationFn: async () => {
      const res = await siteScopedFetch(slug!, "/build");
      if (!res.ok) throw new Error("Build failed");
      const json = await res.json();
      return json;
    },
    onSuccess: () => {
      setBuildLog(prev => [...prev, "Build triggered successfully"]);
    },
  });

  const doDeploy = useMutation({
    mutationFn: async () => {
      const res = await siteScopedFetch(slug!, "/deploy");
      if (!res.ok) throw new Error("Deploy failed");
    },
  });

  return (
    <div>
      <div className="flex items-start justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-foreground">Deploy</h1>
          <p className="text-sm text-muted mt-0.5">Build static site and deploy to production</p>
        </div>
        <Button disabled={doBuild.isPending || doDeploy.isPending}>
          ⇧ Build & Deploy
        </Button>
      </div>

      {/* Last build */}
      <h2 className="text-sm font-semibold text-foreground mb-3">Last Build</h2>
      {isLoading ? (
        <Skeleton className="h-32 w-full" />
      ) : builds && builds.length > 0 ? (
        <div className="border border-line rounded-lg p-5 mb-6">
          <div className="flex items-center justify-between mb-3">
            <span className="font-semibold">Build #{builds[0].id}</span>
            <Badge variant="success">✓ Deployed</Badge>
          </div>
          {builds[0].duration && (
            <div className="flex gap-2 items-center text-xs text-muted mb-4">
              <span className="flex items-center gap-1"><span className="size-2 rounded-full bg-[#22c55e]" /> Build ({builds[0].duration})</span>
              <span className="text-line">→</span>
              <span className="flex items-center gap-1"><span className="size-2 rounded-full bg-[#22c55e]" /> Generate</span>
              <span className="text-line">→</span>
              <span className="flex items-center gap-1"><span className="size-2 rounded-full bg-[#22c55e]" /> Deploy</span>
            </div>
          )}
          {buildLog.length > 0 && (
            <div className="bg-[#1a1e24] text-[#a0a0a0] rounded-md p-3 font-mono text-xs leading-relaxed max-h-32 overflow-y-auto">
              {buildLog.map((line, i) => <div key={i}>{line}</div>)}
            </div>
          )}
        </div>
      ) : (
        <div className="border border-line rounded-lg p-8 text-center text-muted text-sm mb-6">
          No builds yet. Trigger your first build.
        </div>
      )}

      {/* Build history */}
      <h2 className="text-sm font-semibold text-foreground mb-3">Build History</h2>
      <div className="border border-line rounded-lg overflow-hidden">
        <div className="flex bg-surface/50 text-xs font-semibold text-muted uppercase tracking-wider">
          <div className="flex-1 px-4 py-2.5">Build</div>
          <div className="w-24 px-4 py-2.5">Duration</div>
          <div className="w-24 px-4 py-2.5">Status</div>
          <div className="w-32 px-4 py-2.5">Time</div>
        </div>
        {builds && builds.length > 0 ? builds.map((build: any) => (
          <div key={build.id} className="flex border-t border-line text-sm">
            <div className="flex-1 px-4 py-2.5">#{build.id}</div>
            <div className="w-24 px-4 py-2.5 text-muted">{build.duration ?? "—"}</div>
            <div className="w-24 px-4 py-2.5"><Badge variant={build.status === "deployed" ? "success" : "warning"}>{build.status}</Badge></div>
            <div className="w-32 px-4 py-2.5 text-muted text-xs">{build.created_at ?? "—"}</div>
          </div>
        )) : (
          <div className="p-8 text-center text-muted text-sm">No build history</div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create SettingsPage.tsx**

```tsx
import { useState } from "react";
import { useParams } from "react-router";
import { Button } from "../../shared/ui/button";

export function SettingsPage() {
  const { slug } = useParams<{ slug: string }>()!;

  return (
    <div>
      <h1 className="text-xl font-bold text-foreground mb-1">Settings</h1>
      <p className="text-sm text-muted mb-6">Site-wide configuration for {slug}</p>

      <div className="space-y-4">
        {/* General */}
        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">General</h3>
          <SettingsField label="Site Title" defaultValue="My Blog" />
          <SettingsField label="Base URL" defaultValue={`https://${slug}.example.com`} />
          <SettingsField label="Language" defaultValue="ko" type="select" options={["ko", "en", "ko, en"]} />
        </div>

        {/* Display */}
        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">Display</h3>
          <SettingsField label="Default Mode" defaultValue="grid" type="select" options={["grid", "list", "canvas"]} />
          <SettingsField label="Profile" defaultValue="developer" type="select" options={["developer", "writer", "artist"]} />
        </div>

        {/* API Tokens */}
        <div className="border border-line rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4">API Tokens</h3>
          <SettingsField label="TMDB Key" defaultValue="" type="password" placeholder="••••••••••••••••" />
          <SettingsField label="Aladin Key" defaultValue="" type="password" disabled placeholder="not set" />
          <SettingsField label="GitHub User" defaultValue="oxi" />
        </div>

        {/* Danger Zone */}
        <div className="border border-[#fecaca] rounded-lg p-5">
          <h3 className="text-sm font-semibold mb-4 text-[#dc2626]">Danger Zone</h3>
          <div className="flex gap-2">
            <Button variant="destructive" size="sm">Purge All Data</Button>
            <Button variant="destructive" size="sm">Delete Site</Button>
          </div>
        </div>
      </div>

      <div className="flex justify-end gap-2 mt-4">
        <Button variant="outline">Reset</Button>
        <Button>Save Changes</Button>
      </div>
    </div>
  );
}

function SettingsField({
  label, defaultValue, type, options, disabled, placeholder,
}: {
  label: string;
  defaultValue?: string;
  type?: "text" | "password" | "select";
  options?: string[];
  disabled?: boolean;
  placeholder?: string;
}) {
  const [value, setValue] = useState(defaultValue ?? "");
  return (
    <div className="flex items-center gap-3 mb-2.5">
      <label className="text-xs text-muted w-24 shrink-0 text-right">{label}</label>
      {type === "select" ? (
        <select
          value={value}
          onChange={(e) => setValue(e.target.value)}
          className="flex-1 max-w-sm px-2.5 py-1.5 border border-line rounded-md text-sm bg-surface/50"
          disabled={disabled}
        >
          {options?.map((opt) => <option key={opt} value={opt}>{opt}</option>)}
        </select>
      ) : (
        <input
          type={type ?? "text"}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder={placeholder}
          className="flex-1 max-w-sm px-2.5 py-1.5 border border-line rounded-md text-sm bg-surface/50"
          disabled={disabled}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 3: Add config API endpoints to api.ts**

Add to `web/src/admin/shared/api.ts`:

```typescript
export async function getConfig(slug: string): Promise<any> {
  const res = await siteScopedFetch(slug, "/config");
  if (!res.ok) return {};
  return (await res.json()).data ?? {};
}

export async function updateConfig(slug: string, config: any): Promise<Response> {
  return fetch(`/api/console/s/${slug}/config`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(config),
  });
}
```

- [ ] **Step 4: TypeScript check**

Run: `cd web && npx tsc --noEmit`
Expected: Clean

- [ ] **Step 5: Commit**

```bash
git add web/src/admin/deploy/ web/src/admin/settings/ web/src/admin/shared/api.ts
git commit -m "feat(admin): DeployPage (build pipeline + history) and SettingsPage (grouped forms)"
```

---
## Task 6: Clean up old SiteShell + Polish

**Files:**
- Remove: `web/src/admin/shell/SiteShell.tsx` (or keep if referenced elsewhere — check first)
- Modify: `web/src/admin/App.tsx` — remove unused imports

- [ ] **Step 1: Check for references to SiteShell**

Run: `grep -r "SiteShell" web/src/ --include="*.tsx" --include="*.ts"`
If the only reference is in `App.tsx` and it's been replaced by ConsoleShell, delete.

- [ ] **Step 2: Remove unused code**

Delete `web/src/admin/shell/SiteShell.tsx` if unreferenced.

- [ ] **Step 3: TypeScript check**

Run: `cd web && npx tsc --noEmit`
Expected: Clean

- [ ] **Step 4: Dev server smoke test**

Run: `cd web && bun run dev`
Open `http://127.0.0.1:5173/admin.html` (assuming dev proxy to :8787)
Check: sidebar renders, site selector works, navigating between pages works

- [ ] **Step 5: Commit**

```bash
git rm web/src/admin/shell/SiteShell.tsx 2>/dev/null || true
git add -A
git commit -m "feat(admin): remove old SiteShell, full console shell migration complete"
```
