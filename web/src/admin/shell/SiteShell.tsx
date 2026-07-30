import { Link, useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { useState, useEffect } from "react";
import { listSites } from "../shared/api";
import { Container } from "../../shared/ui/container";
import { cn } from "../../shared/ui/cn";
import type { ReactNode } from "react";

function ThemeToggle() {
  const [dark, setDark] = useState(() =>
    document.documentElement.dataset.theme === "dark",
  );

  useEffect(() => {
    const theme = dark ? "dark" : "light";
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem("oxipage-theme", theme);
    } catch { /* noop */ }
  }, [dark]);

  return (
    <button
      type="button"
      onClick={() => setDark((d) => !d)}
      className="inline-flex items-center justify-center size-8 rounded-md text-muted hover:text-foreground hover:bg-surface/50 transition-colors shrink-0"
      aria-label={dark ? "라이트 모드로 전환" : "다크 모드로 전환"}
    >
      {dark ? (
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="5" />
          <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
        </svg>
      ) : (
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
      )}
    </button>
  );
}

export function SiteShell({ children }: { children: ReactNode }) {
  const { slug } = useParams<{ slug?: string }>();
  const { data } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const sites = data?.data ?? [];

  return (
    <div className="min-h-screen bg-canvas flex flex-col">
      <header className="sticky top-0 z-40 bg-canvas/80 backdrop-blur-sm border-b border-line">
        <div className="flex items-center h-14 px-4 gap-6 max-w-screen-xl mx-auto w-full">
          <Link
            to="/"
            className="font-display text-lg font-semibold text-foreground hover:text-primary transition-colors shrink-0"
          >
            Oxipage
          </Link>

          <nav className="flex items-center gap-1 flex-1 min-w-0">
            {sites.map((s) => (
              <Link
                key={s.name}
                to={`/s/${s.name}`}
                className={cn(
                  "px-3 py-1.5 rounded-md text-sm font-medium transition-colors",
                  slug === s.name
                    ? "bg-surface text-foreground"
                    : "text-muted hover:text-foreground hover:bg-surface/50",
                )}
              >
                {s.name}
              </Link>
            ))}
          </nav>

          <ThemeToggle />

          <Link
            to="/sites/new"
            className="inline-flex items-center gap-1 rounded-md bg-primary text-primary-foreground px-3 py-1.5 text-sm font-medium hover:bg-primary/90 transition-colors shrink-0"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M5 12h14" />
              <path d="M12 5v14" />
            </svg>
            새 사이트
          </Link>
        </div>
      </header>

      <main className="flex-1">
        <Container className="py-8">
          {children}
        </Container>
      </main>
    </div>
  );
}
