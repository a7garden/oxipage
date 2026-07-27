import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, ExternalLink } from "lucide-react";
import { Link, useParams } from "react-router";

import { fetchProject } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Markdown } from "../../shared/Markdown";
import { Badge, type badgeVariants } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";
import { Card, CardContent } from "../../shared/ui/card";
import type { VariantProps } from "class-variance-authority";

interface ProjectLinks {
  repo?: string;
  demo?: string;
  app_store?: string;
  play_store?: string;
  custom?: { label: string; url: string }[];
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

export function ProjectDetailPage() {
  const { slug = "" } = useParams();
  const { pick, lang } = useLanguage();
  const { data: project, isLoading, error } = useQuery({
    queryKey: ["projects", slug],
    queryFn: () => fetchProject(slug),
    enabled: !!slug,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (error || !project) {
    return (
      <div className="space-y-4">
        <Button variant="ghost" size="sm" asChild>
          <Link to="/projects">
            <ArrowLeft />
            {lang === "ko" ? "프로젝트" : "Projects"}
          </Link>
        </Button>
        <p className="text-subtle">
          {lang === "ko" ? "프로젝트를 찾을 수 없습니다." : "Project not found."}
        </p>
      </div>
    );
  }

  const title = pick(project.title_ko, project.title_en);
  const description = pick(project.description_ko, project.description_en);
  const links = (project.links ?? {}) as ProjectLinks;
  const linkEntries: { label: string; url: string }[] = [
    links.repo && { label: "Repo", url: links.repo },
    links.demo && { label: "Demo", url: links.demo },
    links.app_store && { label: "App Store", url: links.app_store },
    links.play_store && { label: "Play Store", url: links.play_store },
    ...(links.custom ?? []),
  ].filter((x): x is { label: string; url: string } => !!x);

  return (
    <article className="space-y-6">
      <Button variant="ghost" size="sm" asChild className="-ml-2">
        <Link to="/projects">
          <ArrowLeft />
          {lang === "ko" ? "프로젝트" : "Projects"}
        </Link>
      </Button>

      <Card>
        <div className="space-y-3 p-6">
          <h1 className="font-serif text-3xl font-semibold tracking-tight text-foreground">
            {title}
          </h1>
          <div className="flex flex-wrap items-center gap-2 text-sm text-subtle">
            <Badge variant={STATUS_VARIANT[project.status] ?? "secondary"}>
              {project.status}
            </Badge>
            {project.tech_stack.length > 0 && (
              <span>{project.tech_stack.join(" · ")}</span>
            )}
          </div>
          {linkEntries.length > 0 && (
            <nav className="flex flex-wrap gap-2 pt-1">
              {linkEntries.map((l) => (
                <Button key={l.url} variant="secondary" size="sm" asChild>
                  <a href={l.url} rel="noreferrer noopener">
                    <ExternalLink />
                    {l.label}
                  </a>
                </Button>
              ))}
            </nav>
          )}
        </div>
      </Card>

      {description && (
        <Card>
          <CardContent className="markdown pt-6">
            <Markdown source={description} />
          </CardContent>
        </Card>
      )}

      {project.screenshots.length > 0 && (
        <section className="grid gap-4 sm:grid-cols-2">
          {project.screenshots.map((s) => (
            <figure key={s.id} className="overflow-hidden rounded-lg border border-line bg-surface shadow-sm">
              <img
                src={s.url}
                alt={pick(s.alt_ko, s.alt_en) ?? ""}
                loading="lazy"
                className="block w-full"
              />
            </figure>
          ))}
        </section>
      )}
    </article>
  );
}
