import { useQuery } from "@tanstack/react-query";
import { Film } from "lucide-react";

import { fetchMovies } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Card } from "../../shared/ui/card";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { RatingStars } from "../../shared/RatingStars";

/** TMDB poster_path ("/abc.jpg") → displayable image URL. */
function posterUrl(path: string | null): string | null {
  return path ? `https://image.tmdb.org/t/p/w200${path}` : null;
}

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
        {movies.map((m) => {
          const img = posterUrl(m.poster_path);
          return (
            <li key={m.id}>
              <Card className="flex h-full gap-4 p-4">
                {img ? (
                  <img
                    src={img}
                    alt=""
                    className="w-14 shrink-0 rounded-md object-cover"
                    loading="lazy"
                  />
                ) : (
                  <div className="flex w-14 shrink-0 items-center justify-center rounded-md bg-surface text-subtle">
                    <Film className="size-5" />
                  </div>
                )}
                <div className="min-w-0 space-y-1">
                  <h2 className="font-serif text-base font-semibold leading-tight text-foreground">
                    {m.title}
                  </h2>
                  <div className="flex items-center gap-2 text-xs text-subtle">
                    <span className="uppercase">{m.media_type}</span>
                    {m.release_year && <span>· {m.release_year}</span>}
                  </div>
                  <RatingStars value={m.rating} size="sm" />
                  {pick(m.review_ko, m.review_en) && (
                    <p className="line-clamp-3 text-sm text-subtle">
                      {pick(m.review_ko, m.review_en)}
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
