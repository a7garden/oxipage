import { useQuery } from "@tanstack/react-query";
import { Bookmark } from "lucide-react";

import { fetchScraps } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { ScrapCard } from "./ScrapCard";

export function ScrapsPage() {
  const { pick, lang } = useLanguage();
  const { data: scraps, isLoading } = useQuery({
    queryKey: ["scraps", "list"],
    queryFn: fetchScraps,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!scraps || scraps.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{lang === "ko" ? "스크랩" : "Scraps"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <Bookmark />
          </EmptyStateIcon>
          <EmptyStateTitle>
            {lang === "ko" ? "스크랩이 없습니다" : "No scraps yet"}
          </EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "`oxipage scraps add` 로 스크랩을 추가하세요."
              : "Add a scrap with `oxipage scraps add`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "스크랩" : "Scraps"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {scraps.map((s) => (
          <li key={s.id}>
            <ScrapCard
              scrap={{
                id: s.id,
                title: s.title,
                source_url: s.source_url,
                og_image_url: s.og_image_url,
                note_ko: s.note_ko,
                note_en: s.note_en,
                source: s.source,
                tags: s.tags,
              }}
              pick={pick}
            />
          </li>
        ))}
      </ul>
    </article>
  );
}