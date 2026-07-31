import { useQuery } from "@tanstack/react-query";
import { Film } from "lucide-react";

import { fetchMovies } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { MovieCard } from "./MovieCard";

export function MoviesPage() {
  const { pick, lang } = useLanguage();
  const { data: movies, isLoading } = useQuery({
    queryKey: ["movies", "list"],
    queryFn: fetchMovies,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!movies || movies.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{lang === "ko" ? "영화·드라마" : "Movies & TV"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <Film />
          </EmptyStateIcon>
          <EmptyStateTitle>
            {lang === "ko" ? "기록된 작품이 없습니다" : "No movies yet"}
          </EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "`oxipage movies add` 로 작품을 추가하세요."
              : "Add a title with `oxipage movies add`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "영화·드라마" : "Movies & TV"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {movies.map((m) => (
          <li key={m.id}>
            <MovieCard
              movie={{
                id: m.id,
                title: m.title,
                media_type: m.media_type === "tv" ? "tv" : "movie",
                poster_path: m.poster_path,
                release_year: m.release_year,
                rating: m.rating,
                review_ko: m.review_ko,
                review_en: m.review_en,
              }}
              pick={pick}
            />
          </li>
        ))}
      </ul>
    </article>
  );
}