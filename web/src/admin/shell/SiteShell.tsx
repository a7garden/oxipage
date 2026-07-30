import { Link, useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { listSites } from "../shared/api";
import type { ReactNode } from "react";

export function SiteShell({ children }: { children: ReactNode }) {
  const { slug } = useParams<{ slug?: string }>();
  const { data } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const sites = data?.data ?? [];

  return (
    <div className="admin-shell">
      <header className="admin-topbar">
        <Link to="/" className="admin-logo">Oxipage</Link>
        <nav className="admin-sites-nav">
          {sites.map((s) => (
            <Link
              key={s.name}
              to={`/s/${s.name}`}
              className={slug === s.name ? "active" : ""}
            >
              {s.name}
            </Link>
          ))}
          <Link to="/sites/new" className="new-site-link">+ 새 사이트</Link>
        </nav>
      </header>
      <div className="admin-body">
        <main>{children}</main>
      </div>
    </div>
  );
}
