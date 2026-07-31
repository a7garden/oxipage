import { useQuery } from "@tanstack/react-query";
import { Link2 } from "lucide-react";

import { fetchLinks } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { LinkCard } from "./LinkCard";

export function LinksPage() {
  const { pick, lang } = useLanguage();
  const { data: links, isLoading } = useQuery({
    queryKey: ["links", "list"],
    queryFn: fetchLinks,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!links || links.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{lang === "ko" ? "링크" : "Links"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <Link2 />
          </EmptyStateIcon>
          <EmptyStateTitle>{lang === "ko" ? "등록된 링크가 없습니다" : "No links yet"}</EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "`oxipage links add` 로 링크를 추가하세요."
              : "Add a link with `oxipage links add`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "링크" : "Links"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {links.map((l) => (
          <LinkCard
            key={l.id}
            link={{
              id: l.id,
              url: l.url,
              title: l.title,
              description_ko: l.description_ko,
              description_en: l.description_en,
              thumbnail_url: l.thumbnail_url,
              featured: l.featured,
            }}
            pick={pick}
          />
        ))}
      </ul>
    </article>
  );
}