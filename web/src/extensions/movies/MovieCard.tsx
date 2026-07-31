import { Film } from "lucide-react";

import { Card } from "../../shared/ui/card";
import { RatingStars } from "../../shared/RatingStars";

export interface MovieCardData {
  id: number;
  title: string;
  media_type: "movie" | "tv";
  poster_path: string | null;
  release_year: number | null;
  rating: number;
  review_ko: string | null;
  review_en: string | null;
}

function posterUrl(path: string | null) {
  return path ? `https://image.tmdb.org/t/p/w200${path}` : null;
}

interface MovieCardProps {
  movie: MovieCardData;
  pick: (ko?: string | null, en?: string | null) => string;
}

export function MovieCard({ movie, pick }: MovieCardProps) {
  const img = posterUrl(movie.poster_path);
  return (
    <Card className="flex h-full gap-4 p-4">
      {img ? (
        <img src={img} alt="" className="w-14 shrink-0 rounded-md object-cover" loading="lazy" />
      ) : (
        <div className="flex w-14 shrink-0 items-center justify-center rounded-md bg-surface text-subtle">
          <Film className="size-5" />
        </div>
      )}
      <div className="min-w-0 space-y-1">
        <h2 className="font-serif text-base font-semibold leading-tight text-foreground">{movie.title}</h2>
        <div className="flex items-center gap-2 text-xs text-subtle">
          <span className="uppercase">{movie.media_type}</span>
          {movie.release_year && <span>· {movie.release_year}</span>}
        </div>
        <RatingStars value={movie.rating} size="sm" />
        {pick(movie.review_ko, movie.review_en) && (
          <p className="line-clamp-3 text-sm text-subtle">{pick(movie.review_ko, movie.review_en)}</p>
        )}
      </div>
    </Card>
  );
}