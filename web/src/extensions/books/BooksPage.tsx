import { useQuery } from "@tanstack/react-query";
import { BookOpen } from "lucide-react";

import { fetchBooks } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { BookCard } from "./BookCard";

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
              ? "`oxibuilder books add` 로 책을 추가하세요."
              : "Add a book with `oxibuilder books add`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "책" : "Books"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {books.map((b) => (
          <li key={b.id}>
            <BookCard
              book={{
                id: b.id,
                title: b.title,
                author: b.author,
                cover_image_url: b.cover_image_url,
                rating: b.rating,
                review_ko: b.review_ko,
                review_en: b.review_en,
                status: b.status,
              }}
              pick={pick}
            />
          </li>
        ))}
      </ul>
    </article>
  );
}