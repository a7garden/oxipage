import { useQuery } from "@tanstack/react-query";
import { ExternalLink, Bookmark } from "lucide-react";

import { fetchScraps } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Badge } from "../../shared/ui/badge";
import { Card } from "../../shared/ui/card";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";

function safeHost(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

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
              ? "`oxipage link add` 또는 `oxipage scraps` 로 추가하세요."
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
            <Card className="flex h-full flex-col gap-3 p-4">
              <div className="flex items-start gap-3">
                {s.og_image_url ? (
                  <img
                    src={s.og_image_url}
                    alt=""
                    className="size-12 shrink-0 rounded-md object-cover"
                    loading="lazy"
                  />
                ) : (
                  <div className="flex size-12 shrink-0 items-center justify-center rounded-md bg-surface text-subtle">
                    <ExternalLink className="size-4" />
                  </div>
                )}
                <div className="min-w-0 space-y-1">
                  <a
                    href={s.source_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="font-serif text-base font-semibold leading-tight text-foreground hover:text-primary"
                  >
                    {s.title}
                  </a>
                  <p className="text-xs text-subtle">{safeHost(s.source_url)}</p>
                </div>
              </div>
              {pick(s.note_ko, s.note_en) && (
                <p className="line-clamp-3 text-sm text-subtle">{pick(s.note_ko, s.note_en)}</p>
              )}
              <div className="mt-auto flex items-center gap-2">
                <Badge variant="secondary">{s.source}</Badge>
                {s.tags.length > 0 && (
                  <span className="text-xs text-subtle">#{s.tags.join(" #")}</span>
                )}
              </div>
            </Card>
          </li>
        ))}
      </ul>
    </article>
  );
}
