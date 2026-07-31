import { NavLink, useParams } from "react-router";
import {
  LayoutDashboard, FileText, Puzzle, Palette, Rocket, Settings as SettingsIcon,
} from "lucide-react";

const navGroups = [
  {
    label: "General",
    items: [
      { icon: LayoutDashboard, label: "Dashboard", path: "" },
      { icon: FileText, label: "Content", path: "content" },
      { icon: Puzzle, label: "Extensions", path: "extensions" },
    ],
  },
  {
    label: "Appearance",
    items: [
      { icon: Palette, label: "Themes", path: "themes" },
    ],
  },
  {
    label: "Operations",
    items: [
      { icon: Rocket, label: "Deploy", path: "deploy" },
      { icon: SettingsIcon, label: "Settings", path: "settings" },
    ],
  },
];

export function Sidebar() {
  const { slug } = useParams();

  return (
    <aside className="w-[200px] shrink-0 flex flex-col" style={{ backgroundColor: "var(--console-sidebar-bg)" }}>
      <nav className="flex-1 pt-2">
        {navGroups.map((group) => (
          <div key={group.label}>
            <div className="px-4 pt-4 pb-1.5 text-[10px] font-semibold uppercase tracking-wider" style={{ color: "var(--console-sidebar-label)" }}>
              {group.label}
            </div>
            {group.items.map((item) => {
              const to = item.path ? `/s/${slug}/${item.path}` : `/s/${slug}`;
              const Icon = item.icon;
              return (
                <NavLink
                  key={item.label}
                  to={to}
                  end={item.path === ""}
                  className={({ isActive }) =>
                    `flex items-center gap-2.5 px-4 py-2 text-sm border-l-[3px] transition-all ${
                      isActive ? "" : "hover:text-console-sidebar-text-hover hover:bg-console-sidebar-hover-bg"
                    }`
                  }
                  style={({ isActive }) => ({
                    color: isActive ? "var(--console-sidebar-text-active)" : "var(--console-sidebar-text)",
                    borderLeftColor: isActive ? "var(--console-sidebar-border-active)" : "transparent",
                    backgroundColor: isActive
                      ? "color-mix(in srgb, var(--console-sidebar-border-active) 10%, transparent)"
                      : undefined,
                  })}
                >
                  <Icon size={16} className="opacity-60 shrink-0" />
                  {item.label}
                </NavLink>
              );
            })}
          </div>
        ))}
      </nav>
      <div className="px-4 py-3 border-t border-console-sidebar-border text-xs" style={{ color: "var(--console-sidebar-label)" }}>
        v1.0.0 · {slug ?? "no site"}
      </div>
    </aside>
  );
}
