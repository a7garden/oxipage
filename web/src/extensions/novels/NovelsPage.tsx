import { useQuery } from "@tanstack/react-query";
import { BookOpen } from "lucide-react";

import { fetchNovels } from "../../shared/api";
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

const STATUS_LABEL: Record<string, { ko: string; en: string }> = {
  ongoing: { ko: "연재중", en: "Ongoing" },
  completed: { ko: "완결", en: "Completed" },
  hiatus: { ko: "휴재", en: "Hiatus" },
};

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
              ? "`oxipage novels add` 로 소설을 추가하세요."
              : "Add a novel with `oxipage novels add`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "소설" : "Novels"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {novels.map((n) => {
          const status = STATUS_LABEL[n.status] ?? { ko: n.status, en: n.status };
          return (
            <li key={n.id}>
              <Card className="flex h-full gap-4 p-4">
                {n.cover_image && (
                  <img
                    src={n.cover_image}
                    alt=""
                    className="size-20 shrink-0 rounded-md object-cover"
                    loading="lazy"
                  />
                )}
                <div className="min-w-0 space-y-1">
                  <h2 className="font-serif text-base font-semibold leading-tight text-foreground">
                    {n.title}
                  </h2>
                  <Badge variant="secondary">{pick(status.ko, status.en)}</Badge>
                  {n.synopsis && (
                    <p className="line-clamp-3 text-sm text-subtle">{n.synopsis}</p>
                  )}
                  {n.tags.length > 0 && (
                    <p className="text-xs text-subtle">#{n.tags.join(" #")}</p>
                  )}
                </div>
              </Card>
            </li>
          );
        })}
      </ul>
    </article>
  );
}
