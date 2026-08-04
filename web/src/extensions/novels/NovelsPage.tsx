import { useQuery } from "@tanstack/react-query";
import { BookOpen } from "lucide-react";

import { fetchNovels } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { NovelCard } from "./NovelCard";

export function NovelsPage() {
  const { pick, lang } = useLanguage();
  const { data: novels, isLoading } = useQuery({
    queryKey: ["novels", "list"],
    queryFn: fetchNovels,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!novels || novels.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{lang === "ko" ? "소설" : "Novels"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <BookOpen />
          </EmptyStateIcon>
          <EmptyStateTitle>{lang === "ko" ? "소설이 없습니다" : "No novels yet"}</EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "`oxibuilder novels add` 로 소설을 추가하세요."
              : "Add a novel with `oxibuilder novels add`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "소설" : "Novels"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {novels.map((n) => (
          <li key={n.id}>
            <NovelCard
              novel={{
                id: n.id,
                title: n.title,
                synopsis: n.synopsis,
                cover_image: n.cover_image,
                status: n.status,
                tags: n.tags,
              }}
              pick={pick}
            />
          </li>
        ))}
      </ul>
    </article>
  );
}