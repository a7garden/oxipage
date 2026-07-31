import { Link } from "react-router";
import { Star } from "lucide-react";

import { Card } from "../../shared/ui/card";
import { Badge, type badgeVariants } from "../../shared/ui/badge";
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

export interface ProjectCardData {
  slug: string;
  title_ko: string | null;
  title_en: string | null;
  tech_stack: string[];
  status: string;
  featured: boolean;
}

interface ProjectCardProps {
  project: ProjectCardData;
  pick: (ko?: string | null, en?: string | null) => string;
}

export function ProjectCard({ project, pick }: ProjectCardProps) {
  const title = pick(project.title_ko, project.title_en) ?? project.slug;
  return (
    <li className="relative">
      {project.featured && (
        <Star className="absolute right-3 top-3 z-10 size-4 fill-star text-star" />
      )}
      <Card className={"h-full transition-[border-color,box-shadow] duration-200 hover:border-primary/40 hover:shadow-md " + (project.featured ? "border-primary/50 " : "")}>
        <Link to={`/projects/${project.slug}`} className="block h-full p-5 text-foreground no-underline">
          <h2 className="font-serif text-lg font-semibold tracking-tight">{title}</h2>
          <div className="mt-2">
            <Badge variant={STATUS_VARIANT[project.status] ?? "secondary"}>
              {project.status}
            </Badge>
          </div>
          {project.tech_stack.length > 0 && (
            <p className="mt-2 text-sm text-subtle">{project.tech_stack.join(" · ")}</p>
          )}
        </Link>
      </Card>
    </li>
  );
}