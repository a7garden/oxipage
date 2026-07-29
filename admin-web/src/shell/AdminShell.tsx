// Admin Shell layout — sidebar + topbar + content area, with site context management.

import { useEffect, useState } from "react";
import { Outlet, Link, useLocation } from "react-router";
import { LayoutDashboard, Puzzle, FileText, Table2, Palette, Settings, Logs } from "lucide-react";
import { SiteContext, type SiteProfile, listSites, setActiveSite as apiSetActive } from "../shared/api";
import { ThemeToggle } from "../shared/ThemeToggle";

export function AdminShell() {
  const [sites, setSites] = useState<SiteProfile[]>([]);
  const [activeSite, setActiveSiteState] = useState<SiteProfile | null>(null);

  const refreshSites = async () => {
    try {
      const { data } = await listSites();
      // v2 SSG: always have a default local site so UI gating works
      const localDefault: SiteProfile = { name: 'local', endpoint: 'http://127.0.0.1:8787' };
      const all = data.length > 0 ? data : [localDefault];
      setSites(all);
      const active = all.find((s) => s.name === 'local') || all[0] || null;
      setActiveSiteState(active);
    } catch {
      // No sites.toml yet — show local default
      setSites([{ name: 'local', endpoint: 'http://127.0.0.1:8787' }]);
      setActiveSiteState({ name: 'local', endpoint: 'http://127.0.0.1:8787' });
    }
  };

  useEffect(() => {
    refreshSites();
  }, []);

  const setActive = async (name: string) => {
    await apiSetActive(name);
    await refreshSites();
  };

  return (
    <SiteContext.Provider value={{ activeSite, setActiveSite: setActive, sites, refreshSites }}>
      <div className="admin-layout">
        <Sidebar />
        <div className="admin-main">
          <TopBar />
          <main className="admin-content">
            <Outlet />
          </main>
        </div>
      </div>
    </SiteContext.Provider>
  );
}

// ─── Sidebar ───

const NAV_ITEMS = [
  { path: "/", label: "대시보드", icon: LayoutDashboard, exact: true },
  { path: "/extensions", label: "확장", icon: Puzzle },
  { path: "/content/blog", label: "블로그", icon: FileText },
  { path: "/content/scraps", label: "데이터", icon: Table2 },
  { path: "/themes", label: "테마", icon: Palette },
  { path: "/settings", label: "설정", icon: Settings },
];

function Sidebar() {
  const location = useLocation();

  const isActive = (item: (typeof NAV_ITEMS)[number]) => {
    if (item.exact) return location.pathname === item.path;
    return location.pathname.startsWith(item.path);
  };

  return (
    <aside className="admin-sidebar">
      <div className="logo">Oxipage Studio</div>
      <nav className="nav-section">
        {NAV_ITEMS.map((item) => {
          const Icon = item.icon;
          return (
            <Link
              key={item.path}
              to={item.path}
              className={`nav-item ${isActive(item) ? "active" : ""}`}
            >
              <Icon size={16} />
              <span>{item.label}</span>
            </Link>
          );
        })}
      </nav>
    </aside>
  );
}

// ─── TopBar with SiteSwitcher ───

import { SiteSwitcher } from "./SiteSwitcher";

function TopBar() {
  return (
    <header className="admin-topbar">
      <div className="flex items-center gap-3">
        <SiteSwitcher />
      </div>
      <div className="flex items-center gap-2 text-xs text-muted">
        <Logs size={14} />
        <span className="hidden sm:inline">Local Admin</span>
        <ThemeToggle />
      </div>
    </header>
  );
}
