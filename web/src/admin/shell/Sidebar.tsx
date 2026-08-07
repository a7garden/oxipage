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

/**
 * Console sidebar — the chrome anchor of the admin shell. Colors resolve
 * exclusively through the --console-sidebar-* + --console-active-* tokens
 * so light/dark stay in lock-step with the rest of the OKLCH system.
 */
export function Sidebar() {
  const { slug } = useParams();

  return (
    <aside
      className="w-[200px] shrink-0 flex flex-col bg-console-sidebar-bg"
    >
      <nav className="flex-1 pt-2">
        {navGroups.map((group) => (
          <div key={group.label}>
            <div className="px-4 pt-4 pb-1.5 text-[10px] font-semibold uppercase tracking-wider text-console-sidebar-label">
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
                    [
                      "flex items-center gap-2.5 px-4 py-2 text-sm border-l-[3px] transition-all",
                      isActive
                        ? "text-console-sidebar-text-active bg-active-soft"
                        : "text-console-sidebar-text hover:text-console-sidebar-text-hover hover:bg-console-sidebar-hover-bg",
                    ].join(" ")
                  }
                  style={({ isActive }) => ({
                    borderLeftColor: isActive ? "var(--console-active-line)" : "transparent",
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
      <div className="px-4 py-3 border-t border-console-sidebar-border text-xs text-console-sidebar-label">
        v1.0.0 · {slug ?? "no site"}
      </div>
    </aside>
  );
}
