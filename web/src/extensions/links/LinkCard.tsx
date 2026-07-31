import { ExternalLink, Star } from "lucide-react";

import { Card } from "../../shared/ui/card";

export interface LinkCardData {
  id: number;
  url: string;
  title: string;
  description_ko: string | null;
  description_en: string | null;
  thumbnail_url: string | null;
  featured: boolean;
}

interface LinkCardProps {
  link: LinkCardData;
  pick: (ko?: string | null, en?: string | null) => string;
}

function safeHost(url: string) {
  try { return new URL(url).host; } catch { return url; }
}

export function LinkCard({ link, pick }: LinkCardProps) {
  const description = pick(link.description_ko, link.description_en);
  return (
    <li className="relative">
      {link.featured && (
        <Star className="absolute right-3 top-3 z-10 size-4 fill-star text-star" />
      )}
      <Card className={"h-full transition-[border-color,box-shadow] duration-200 hover:border-primary/40 hover:shadow-md " + (link.featured ? "border-primary/50 " : "")}>
        <a href={link.url} rel="noreferrer noopener" className="flex h-full gap-3 p-4 text-foreground no-underline">
          {link.thumbnail_url && (
            <img src={link.thumbnail_url} alt="" loading="lazy" className="size-16 shrink-0 rounded-md border border-line object-cover" />
          )}
          <div className="min-w-0 flex-1">
            <h2 className="truncate font-medium text-foreground">{link.title}</h2>
            {description && <p className="mt-0.5 line-clamp-2 text-sm text-muted">{description}</p>}
            <span className="mt-1 inline-flex items-center gap-1 text-xs text-subtle">
              <ExternalLink className="size-3" />
              {safeHost(link.url)}
            </span>
          </div>
        </a>
      </Card>
    </li>
  );
}