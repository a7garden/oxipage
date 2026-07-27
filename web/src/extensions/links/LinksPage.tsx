import { useQuery } from "@tanstack/react-query";
import { ExternalLink, Link2, Star } from "lucide-react";

import { fetchLinks } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Card } from "../../shared/ui/card";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";

export function LinksPage() {
  const { pick, lang } = useLanguage();
  const { data: links, isLoading } = useQuery({
    queryKey: ["links", "list"],
    queryFn: fetchLinks,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!links || links.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{lang === "ko" ? "링크" : "Links"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <Link2 className="size-5" />
          </EmptyStateIcon>
          <EmptyStateTitle>
            {lang === "ko" ? "아직 링크가 없습니다" : "No links yet"}
          </EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "수집한 링크가 여기 표시됩니다."
              : "Collected links will appear here."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "링크" : "Links"}</PageTitle>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {links.map((l) => {
          const description = pick(l.description_ko, l.description_en);
          return (
            <li key={l.id} className="relative">
              {l.featured && (
                <Star className="absolute right-3 top-3 z-10 size-4 fill-star text-star" />
              )}
              <Card
                className={
                  "h-full transition-[border-color,box-shadow] duration-200 hover:border-primary/40 hover:shadow-md " +
                  (l.featured ? "border-primary/50 " : "")
                }
              >
                <a
                  href={l.url}
                  rel="noreferrer noopener"
                  className="flex h-full gap-3 p-4 text-foreground no-underline"
                >
                  {l.thumbnail_url && (
                    <img
                      src={l.thumbnail_url}
                      alt=""
                      loading="lazy"
                      className="size-16 shrink-0 rounded-md border border-line object-cover"
                    />
                  )}
                  <div className="min-w-0 flex-1">
                    <h2 className="truncate font-medium text-foreground">
                      {l.title}
                    </h2>
                    {description && (
                      <p className="mt-0.5 line-clamp-2 text-sm text-muted">
                        {description}
                      </p>
                    )}
                    <span className="mt-1 inline-flex items-center gap-1 text-xs text-subtle">
                      <ExternalLink className="size-3" />
                      {safeHost(l.url)}
                    </span>
                  </div>
                </a>
              </Card>
            </li>
          );
        })}
      </ul>
    </article>
  );
}

function safeHost(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}
