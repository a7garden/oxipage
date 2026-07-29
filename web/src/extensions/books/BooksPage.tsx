import { useQuery } from "@tanstack/react-query";
import { BookOpen } from "lucide-react";

import { fetchBooks } from "../../shared/api";
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
import { RatingStars } from "../../shared/RatingStars";

const STATUS_LABEL: Record<string, { ko: string; en: string }> = {
  wishlist: { ko: "읽고 싶음", en: "Wishlist" },
  reading: { ko: "읽는중", en: "Reading" },
  completed: { ko: "완독", en: "Completed" },
  dropped: { ko: "중단", en: "Dropped" },
};

export function BooksPage() {
  const { pick, lang } = useLanguage();
  const { data: books, isLoading } = useQuery({
    queryKey: ["books", "list"],
    queryFn: fetchBooks,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!books || books.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{lang === "ko" ? "책" : "Books"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <BookOpen />
          </EmptyStateIcon>
          <EmptyStateTitle>{lang === "ko" ? "책이 없습니다" : "No books yet"}</EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "`oxipage books add` 로 책을 추가하세요."
              : "Add a book with `oxipage books add`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "책" : "Books"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {books.map((b) => {
          const status = STATUS_LABEL[b.status] ?? { ko: b.status, en: b.status };
          return (
            <li key={b.id}>
              <Card className="flex h-full gap-4 p-4">
                {b.cover_image_url ? (
                  <img
                    src={b.cover_image_url}
                    alt=""
                    className="w-14 shrink-0 rounded-md object-cover"
                    loading="lazy"
                  />
                ) : (
                  <div className="flex w-14 shrink-0 items-center justify-center rounded-md bg-surface text-subtle">
                    <BookOpen className="size-5" />
                  </div>
                )}
                <div className="min-w-0 space-y-1">
                  <h2 className="font-serif text-base font-semibold leading-tight text-foreground">
                    {b.title}
                  </h2>
                  {b.author && <p className="text-xs text-subtle">{b.author}</p>}
                  <div className="flex items-center gap-2">
                    <RatingStars value={b.rating} size="sm" />
                    <Badge variant="secondary">{pick(status.ko, status.en)}</Badge>
                  </div>
                  {pick(b.review_ko, b.review_en) && (
                    <p className="line-clamp-3 text-sm text-subtle">
                      {pick(b.review_ko, b.review_en)}
                    </p>
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
