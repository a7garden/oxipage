import { ExternalLink } from "lucide-react";

import { Card } from "../../shared/ui/card";
import { Badge } from "../../shared/ui/badge";

export interface ScrapCardData {
  id: number;
  title: string;
  source_url: string;
  og_image_url: string | null;
  note_ko: string | null;
  note_en: string | null;
  source: string;
  tags: string[];
}

function safeHost(url: string) {
  try { return new URL(url).host; } catch { return url; }
}

interface ScrapCardProps {
  scrap: ScrapCardData;
  pick: (ko?: string | null, en?: string | null) => string;
}

export function ScrapCard({ scrap, pick }: ScrapCardProps) {
  return (
    <Card className="flex h-full flex-col gap-3 p-4">
      <div className="flex items-start gap-3">
        {scrap.og_image_url ? (
          <img src={scrap.og_image_url} alt="" className="size-12 shrink-0 rounded-md object-cover" loading="lazy" />
        ) : (
          <div className="flex size-12 shrink-0 items-center justify-center rounded-md bg-surface text-subtle">
            <ExternalLink className="size-4" />
          </div>
        )}
        <div className="min-w-0 space-y-1">
          <a href={scrap.source_url} target="_blank" rel="noopener noreferrer" className="font-serif text-base font-semibold leading-tight text-foreground hover:text-primary">
            {scrap.title}
          </a>
          <p className="text-xs text-subtle">{safeHost(scrap.source_url)}</p>
        </div>
      </div>
      {pick(scrap.note_ko, scrap.note_en) && (
        <p className="line-clamp-3 text-sm text-subtle">{pick(scrap.note_ko, scrap.note_en)}</p>
      )}
      <div className="mt-auto flex items-center gap-2">
        <Badge variant="secondary">{scrap.source}</Badge>
        {scrap.tags.length > 0 && <span className="text-xs text-subtle">#{scrap.tags.join(" #")}</span>}
      </div>
    </Card>
  );
}