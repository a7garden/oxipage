import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { Sparkles } from "lucide-react";

import { fetchManifest } from "../shared/api";
import { useLanguage } from "../shared/language";
import { LangToggle } from "../shared/LangToggle";
import { ThemeToggle } from "../shared/ThemeToggle";
import { EXT_ICONS, displayName, mountName } from "./Lobby";

/**
 * Editorial hub — a centered site identity plus a grid of section cards.
 * Replaces the regular Lobby when the layout variant is `editorial`.
 * Each page renders its own EditorialPageHeader, so there is no chrome here
 * beyond the ThemeToggle + LangToggle control row at the bottom.
 */
export function HubLobby() {
  const { data: manifest } = useQuery({
    queryKey: ["manifest"],
    queryFn: fetchManifest,
  });
  const { lang } = useLanguage();

  if (!manifest) {
    return (
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {Array.from({ length: 6 }).map((_, i) => (
          <div
            key={i}
            className="h-28 animate-pulse rounded-lg border border-line bg-surface"
          />
        ))}
      </div>
    );
  }

  const siteName = manifest.site.name;
  const exts = [...manifest.extensions]
    .filter((e) => e.lobby.enabled)
    .sort((a, b) => a.lobby.display_order - b.lobby.display_order);

  return (
    <div className="space-y-12">
      {/* Identity */}
      <header className="text-center">
        <h1 className="font-serif text-4xl font-semibold tracking-tight text-foreground sm:text-5xl">
          {siteName}
        </h1>
      </header>

      {/* Section cards */}
      <section
        aria-label={lang === "ko" ? "섹션" : "Sections"}
        className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3"
      >
        {exts.map((ext) => {
          const Icon = EXT_ICONS[ext.id] ?? Sparkles;
          const name = displayName(ext, lang);
          return (
            <Link
              key={ext.id}
              to={`/${ext.id}`}
              data-extension={ext.id}
              className="group flex flex-col gap-3 rounded-lg border border-line bg-surface p-5 transition-colors duration-200 ease-out hover:border-foreground/30 hover:bg-canvas"
            >
              <div className="flex size-10 items-center justify-center rounded-md bg-primary/10 text-primary">
                <Icon className="size-5" />
              </div>
              <div className="space-y-0.5">
                <h2 className="font-serif text-lg font-semibold leading-tight tracking-tight text-foreground">
                  {name}
                </h2>
                <p className="text-sm text-subtle">/{ext.id}</p>
              </div>
            </Link>
          );
        })}
        {manifest.mounts.map((m) => (
          <a
            key={m.id}
            href={`${m.path}/`}
            data-mount={m.id}
            {...(m.open_in_new_tab
              ? { target: "_blank", rel: "noopener" }
              : {})}
            className="group flex flex-col gap-3 rounded-lg border border-line bg-surface p-5 transition-colors duration-200 ease-out hover:border-foreground/30 hover:bg-canvas"
          >
            <div className="flex size-10 items-center justify-center rounded-md bg-primary/10 text-primary text-lg">
              <span aria-hidden>{m.icon ?? "🔗"}</span>
            </div>
            <div className="space-y-0.5">
              <h2 className="font-serif text-lg font-semibold leading-tight tracking-tight text-foreground">
                {mountName(m, lang)}
              </h2>
              {m.description && (
                <p className="text-sm text-subtle">{m.description}</p>
              )}
            </div>
          </a>
        ))}
      </section>

      {/* Control row — replaces the header chrome */}
      <footer className="flex items-center justify-center gap-1 border-t border-line pt-6">
        <LangToggle />
        <ThemeToggle />
      </footer>
    </div>
  );
}
