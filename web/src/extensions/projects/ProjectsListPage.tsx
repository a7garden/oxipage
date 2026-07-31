import { useQuery } from "@tanstack/react-query";
import { FolderGit2 } from "lucide-react";

import { fetchProjects } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { ProjectCard } from "./ProjectCard";

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
            <FolderGit2 />
          </EmptyStateIcon>
          <EmptyStateTitle>
            {lang === "ko" ? "프로젝트가 없습니다" : "No projects yet"}
          </EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "`oxipage projects add` 로 프로젝트를 추가하세요."
              : "Add a project with `oxipage projects add`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "프로젝트" : "Projects"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {projects.map((p) => (
          <ProjectCard
            key={p.slug}
            project={{
              slug: p.slug,
              title_ko: p.title_ko,
              title_en: p.title_en,
              tech_stack: p.tech_stack,
              status: p.status,
              featured: p.featured,
            }}
            pick={pick}
          />
        ))}
      </ul>
    </article>
  );
}