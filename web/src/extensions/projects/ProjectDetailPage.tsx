import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { Link, useParams } from "react-router";

import { fetchProject } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Button } from "../../shared/ui/button";
import { ProjectView, type ProjectViewData } from "./ProjectView";

export function ProjectDetailPage() {
  const { slug = "" } = useParams();
  const { pick, lang } = useLanguage();
  const { data: project, isLoading, error } = useQuery<ProjectViewData | null>({
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

  return (
    <div className="space-y-6">
      <Button variant="ghost" size="sm" asChild className="-ml-2">
        <Link to="/projects">
          <ArrowLeft />
          {lang === "ko" ? "프로젝트" : "Projects"}
        </Link>
      </Button>
      <ProjectView project={project} pick={pick} />
    </div>
  );
}