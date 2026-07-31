import { ExternalLink } from "lucide-react";

import { Markdown } from "../../shared/Markdown";
import { Badge, type badgeVariants } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";
import { Card, CardContent } from "../../shared/ui/card";
import type { VariantProps } from "class-variance-authority";

export interface ProjectScreenshot {
  id: number;
  url: string;
  alt_ko: string | null;
  alt_en: string | null;
}

interface ProjectLinks {
  repo?: string;
  demo?: string;
  app_store?: string;
  play_store?: string;
  custom?: { label: string; url: string }[];
}

export interface ProjectViewData {
  title_ko: string | null;
  title_en: string | null;
  description_ko: string | null;
  description_en: string | null;
  tech_stack: string[];
  status: string;
  started_at: string | null;
  ended_at: string | null;
  links: ProjectLinks | unknown;
  screenshots: ProjectScreenshot[];
}

interface ProjectViewProps {
  project: ProjectViewData;
  pick: (ko?: string | null, en?: string | null) => string;
}

type BadgeVariant = NonNullable<VariantProps<typeof badgeVariants>["variant"]>;
const STATUS_VARIANT: Record<string, BadgeVariant> = {
  active: "positive",
  shipped: "positive",
  wip: "accent",
  planning: "secondary",
  paused: "secondary",
  archived: "secondary",
};

export function ProjectView({ project, pick }: ProjectViewProps) {
  const title = pick(project.title_ko, project.title_en) ?? "";
  const description = pick(project.description_ko, project.description_en);
  const links = (project.links ?? {}) as ProjectLinks;
  const linkEntries = [
    links.repo && { label: "Repo", url: links.repo },
    links.demo && { label: "Demo", url: links.demo },
    links.app_store && { label: "App Store", url: links.app_store },
    links.play_store && { label: "Play Store", url: links.play_store },
    ...(links.custom ?? []),
  ].filter((x): x is { label: string; url: string } => !!x);

  return (
    <article className="space-y-6">
      <Card>
        <div className="space-y-3 p-6">
          <h1 className="font-serif text-3xl font-semibold tracking-tight text-foreground">{title}</h1>
          <div className="flex flex-wrap items-center gap-2 text-sm text-subtle">
            <Badge variant={STATUS_VARIANT[project.status] ?? "secondary"}>{project.status}</Badge>
            {project.tech_stack.length > 0 && <span>{project.tech_stack.join(" · ")}</span>}
          </div>
          {(project.started_at || project.ended_at) && (
            <p className="text-xs text-subtle">
              {project.started_at ?? "?"} – {project.ended_at ?? "present"}
            </p>
          )}
          {linkEntries.length > 0 && (
            <nav className="flex flex-wrap gap-2 pt-1">
              {linkEntries.map((l) => (
                <Button key={l.url} variant="secondary" size="sm" asChild>
                  <a href={l.url} rel="noreferrer noopener"><ExternalLink />{l.label}</a>
                </Button>
              ))}
            </nav>
          )}
        </div>
      </Card>

      {description && (
        <Card>
          <CardContent className="markdown pt-6"><Markdown source={description} /></CardContent>
        </Card>
      )}

      {project.screenshots.length > 0 && (
        <section className="grid gap-4 sm:grid-cols-2">
          {project.screenshots.map((s) => (
            <figure key={s.id} className="overflow-hidden rounded-lg border border-line bg-surface shadow-sm">
              <img src={s.url} alt={pick(s.alt_ko, s.alt_en) ?? ""} loading="lazy" className="block w-full" />
            </figure>
          ))}
        </section>
      )}
    </article>
  );
}