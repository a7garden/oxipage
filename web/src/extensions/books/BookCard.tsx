import { BookOpen } from "lucide-react";

import { Card } from "../../shared/ui/card";
import { Badge } from "../../shared/ui/badge";
import { RatingStars } from "../../shared/RatingStars";
import { pickVariant, useOptimizedImage } from "../../shared/useOptimizedImage";

const EAGER_COUNT = 10;

export interface BookCardData {
  id: number;
  title: string;
  author: string | null;
  cover_image_url: string | null;
  rating: number;
  review_ko: string | null;
  review_en: string | null;
  status: string;
}

interface BookCardProps {
  book: BookCardData;
  index?: number;
  pick: (ko?: string | null, en?: string | null) => string;
}

const STATUS_LABEL: Record<string, { ko: string; en: string }> = {
  wishlist: { ko: "읽고 싶음", en: "Wishlist" },
  reading: { ko: "읽는중", en: "Reading" },
  completed: { ko: "완독", en: "Completed" },
  dropped: { ko: "중단", en: "Dropped" },
};

export function BookCard({ book, index, pick }: BookCardProps) {
  const status = STATUS_LABEL[book.status] ?? { ko: book.status, en: book.status };
  const optimized = useOptimizedImage(book.cover_image_url);
  const variant = optimized ? pickVariant(optimized) : null;
  const eager = index != null && index < EAGER_COUNT;
  return (
    <Card className="flex h-full gap-4 p-4">
      {variant ? (
        <img
          src={variant.url}
          srcSet={optimized!.srcset.map((s) => `${s.url} ${s.w}w`).join(", ")}
          sizes="56px"
          width={optimized!.width}
          height={optimized!.height}
          alt=""
          loading={eager ? "eager" : "lazy"}
          fetchPriority={eager ? "high" : "auto"}
          decoding="async"
          className="w-14 shrink-0 rounded-md object-cover"
        />
      ) : book.cover_image_url ? (
        <img
          src={book.cover_image_url}
          alt=""
          className="w-14 shrink-0 rounded-md object-cover"
          loading={eager ? "eager" : "lazy"}
          fetchPriority={eager ? "high" : "auto"}
        />
      ) : (
        <div className="flex w-14 shrink-0 items-center justify-center rounded-md bg-surface text-subtle">
          <BookOpen className="size-5" />
        </div>
      )}
      <div className="min-w-0 space-y-1">
        <h2 className="font-serif text-base font-semibold leading-tight text-foreground">{book.title}</h2>
        {book.author && <p className="text-xs text-subtle">{book.author}</p>}
        <div className="flex items-center gap-2">
          <RatingStars value={book.rating} size="sm" />
          <Badge variant="secondary">{pick(status.ko, status.en) ?? status.en}</Badge>
        </div>
        {pick(book.review_ko, book.review_en) && (
          <p className="line-clamp-3 text-sm text-subtle">{pick(book.review_ko, book.review_en)}</p>
        )}
      </div>
    </Card>
  );
}