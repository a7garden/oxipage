import { useQuery } from "@tanstack/react-query";
import { FolderGit2, Star } from "lucide-react";
import { Link } from "react-router";

import { fetchProjects } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Badge, type badgeVariants } from "../../shared/ui/badge";
import { Card } from "../../shared/ui/card";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import type { VariantProps } from "class-variance-authority";

type BadgeVariant = NonNullable<VariantProps<typeof badgeVariants>["variant"]>;
const STATUS_VARIANT: Record<string, BadgeVariant> = {
  active: "positive",
  shipped: "positive",
  wip: "accent",
  planning: "secondary",
  paused: "secondary",
  archived: "secondary",
};

export function ProjectsListPage() {
  const { pick, lang } = useLanguage();
  const { data: projects, isLoading } = useQuery({
    queryKey: ["projects", "list"],
    queryFn: fetchProjects,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!projects || projects.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{lang === "ko" ? "프로젝트" : "Projects"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <FolderGit2 className="size-5" />
          </EmptyStateIcon>
          <EmptyStateTitle>
            {lang === "ko" ? "아직 프로젝트가 없습니다" : "No projects yet"}
          </EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "곧 작업물이 공개됩니다."
              : "Work will be shared here soon."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "프로젝트" : "Projects"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {projects.map((p) => {
          const title = pick(p.title_ko, p.title_en);
          return (
            <li key={p.slug} className="relative">
              {p.featured && (
                <Star className="absolute right-3 top-3 z-10 size-4 fill-star text-star" />
              )}
              <Card
                className={
                  "h-full transition-[border-color,box-shadow] duration-200 hover:border-primary/40 hover:shadow-md " +
                  (p.featured ? "border-primary/50 " : "")
                }
              >
                <Link
                  to={`/projects/${p.slug}`}
                  className="block h-full p-5 text-foreground no-underline"
                >
                  <h2 className="font-serif text-lg font-semibold tracking-tight">
                    {title}
                  </h2>
                  <div className="mt-2">
                    <Badge variant={STATUS_VARIANT[p.status] ?? "secondary"}>
                      {p.status}
                    </Badge>
                  </div>
                  {p.tech_stack.length > 0 && (
                    <p className="mt-2 text-sm text-subtle">
                      {p.tech_stack.join(" · ")}
                    </p>
                  )}
                </Link>
              </Card>
            </li>
          );
        })}
      </ul>
    </article>
  );
}
