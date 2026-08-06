import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, Search, X } from "lucide-react";

import { fetchBooks } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { Input } from "../../shared/ui/input";
import { Link } from "react-router";
import { useCollectionFilter } from "../../shared/useCollectionFilter";
import { BookCard } from "./BookCard";

const selectCls =
  "h-9 rounded-md border border-line bg-canvas px-2.5 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-primary";

const STATUSES = ["wishlist", "reading", "completed", "dropped"] as const;

function statusLabel(status: string, ko: boolean): string {
  const map: Record<string, [string, string]> = {
    wishlist: ["읽고 싶다", "Wishlist"],
    reading: ["읽는 중", "Reading"],
    completed: ["읽음", "Completed"],
    dropped: ["중단", "Dropped"],
  };
  const e = map[status];
  return e ? (ko ? e[0] : e[1]) : status;
}

export function BooksPage() {
  const { pick, lang } = useLanguage();
  const ko = lang === "ko";
  const { data: books, isLoading } = useQuery({
    queryKey: ["books", "list"],
    queryFn: fetchBooks,
  });

  const [status, setStatus] = useState<string | null>(null);

  const statusCounts = useMemo(() => {
    const m = new Map<string, number>();
    for (const b of books ?? []) if (b.status) m.set(b.status, (m.get(b.status) ?? 0) + 1);
    return m;
  }, [books]);

  const visible = useMemo(
    () => (status ? (books ?? []).filter((b) => b.status === status) : books ?? []),
    [books, status],
  );

  const { query, setQuery, sort, setSort, filtered } = useCollectionFilter(visible, {
    matches: (b, q) => `${b.title} ${b.author ?? ""}`.toLowerCase().includes(q),
    sortFns: {
      recent: (a, b) => b.created_at.localeCompare(a.created_at),
      rating: (a, b) => b.rating - a.rating,
      title: (a, b) => a.title.localeCompare(b.title, ko ? "ko" : "en"),
    },
    initialSort: "recent",
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!books || books.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{ko ? "책" : "Books"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <BookOpen />
          </EmptyStateIcon>
          <EmptyStateTitle>{ko ? "책이 없습니다" : "No books yet"}</EmptyStateTitle>
          <EmptyStateDescription>
            {ko ? "`oxibuilder books add` 로 책을 추가하세요." : "Add a book with `oxibuilder books add`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  const hasFilters = !!query || status != null;
  const clearAll = () => {
    setQuery("");
    setStatus(null);
    setSort("recent");
  };

  return (
    <article className="space-y-6">
      <div className="flex items-end justify-between gap-4">
        <PageTitle>{ko ? "책" : "Books"}</PageTitle>
        <div className="flex items-center gap-4 pb-2 text-sm text-subtle">
          <span>{filtered.length}</span>
          <Link to="/books/stats" className="transition-colors hover:text-foreground">
            {ko ? "통계" : "Stats"}
          </Link>
        </div>
      </div>

      {/* Filter bar */}
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-subtle" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={ko ? "제목 · 저자" : "Title · Author"}
            className="h-9 w-56 pl-8"
          />
        </div>
        {STATUSES.filter((s) => statusCounts.has(s)).map((s) => (
          <button
            key={s}
            type="button"
            onClick={() => setStatus(status === s ? null : s)}
            className={
              "rounded-full border px-3 py-1 text-sm transition-colors " +
              (status === s
                ? "border-primary bg-primary/10 text-foreground"
                : "border-line text-subtle hover:text-foreground")
            }
          >
            {statusLabel(s, ko)} <span className="text-subtle">{statusCounts.get(s)}</span>
          </button>
        ))}
        <select
          value={sort}
          onChange={(e) => setSort(e.target.value)}
          className={selectCls}
          aria-label={ko ? "정렬" : "Sort"}
        >
          <option value="recent">{ko ? "최신순" : "Recent"}</option>
          <option value="rating">{ko ? "평점순" : "Rating"}</option>
          <option value="title">{ko ? "제목순" : "Title"}</option>
        </select>
        {hasFilters && (
          <button type="button" onClick={clearAll} className="text-sm text-subtle transition-colors hover:text-foreground">
            {ko ? "전체 해제" : "Clear"}
          </button>
        )}
      </div>

      {/* Active filter chips */}
      {status != null && (
        <div className="flex flex-wrap items-center gap-2">
          <FilterChip label={statusLabel(status, ko)} onClear={() => setStatus(null)} />
        </div>
      )}

      {filtered.length === 0 ? (
        <p className="text-subtle">{ko ? "결과가 없습니다." : "No matches."}</p>
      ) : (
        <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filtered.map((b) => (
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
      )}
    </article>
  );
}

function FilterChip({ label, onClear }: { label: string; onClear: () => void }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full border border-line bg-surface px-3 py-1 text-sm text-foreground">
      {label}
      <button type="button" onClick={onClear} aria-label="clear" className="text-subtle hover:text-foreground">
        <X className="size-3.5" />
      </button>
    </span>
  );
}
