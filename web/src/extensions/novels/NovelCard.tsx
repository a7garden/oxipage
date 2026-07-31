import { Card } from "../../shared/ui/card";
import { Badge } from "../../shared/ui/badge";

export interface NovelCardData {
  id: number;
  title: string;
  synopsis: string | null;
  cover_image: string | null;
  status: string;
  tags: string[];
}

const STATUS_LABEL: Record<string, { ko: string; en: string }> = {
  ongoing: { ko: "연재중", en: "Ongoing" },
  completed: { ko: "완결", en: "Completed" },
  hiatus: { ko: "휴재", en: "Hiatus" },
};

interface NovelCardProps {
  novel: NovelCardData;
  pick: (ko?: string | null, en?: string | null) => string;
}

export function NovelCard({ novel, pick }: NovelCardProps) {
  const status = STATUS_LABEL[novel.status] ?? { ko: novel.status, en: novel.status };
  return (
    <Card className="flex h-full gap-4 p-4">
      {novel.cover_image && (
        <img src={novel.cover_image} alt="" className="size-20 shrink-0 rounded-md object-cover" loading="lazy" />
      )}
      <div className="min-w-0 space-y-1">
        <h2 className="font-serif text-base font-semibold leading-tight text-foreground">{novel.title}</h2>
        <Badge variant="secondary">{pick(status.ko, status.en) ?? status.en}</Badge>
        {novel.synopsis && <p className="line-clamp-3 text-sm text-subtle">{novel.synopsis}</p>}
        {novel.tags.length > 0 && <p className="text-xs text-subtle">#{novel.tags.join(" #")}</p>}
      </div>
    </Card>
  );
}