import { useQuery } from "@tanstack/react-query";
import { Activity, ExternalLink } from "lucide-react";

import { fetchActivity } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Badge } from "../../shared/ui/badge";
import {
  EmptyState,
  EmptyStateDescription,
  EmptyStateIcon,
  EmptyStateTitle,
} from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";

function relativeTime(iso: string, lang: "ko" | "en"): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;
  const diff = Date.now() - then;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return lang === "ko" ? "방금" : "just now";
  if (mins < 60) return lang === "ko" ? `${mins}분 전` : `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return lang === "ko" ? `${hours}시간 전` : `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return lang === "ko" ? `${days}일 전` : `${days}d ago`;
}

export function ActivityPage() {
  const { lang } = useLanguage();
  const { data: events, isLoading } = useQuery({
    queryKey: ["activity", "list"],
    queryFn: fetchActivity,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!events || events.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{lang === "ko" ? "활동" : "Activity"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <Activity />
          </EmptyStateIcon>
          <EmptyStateTitle>
            {lang === "ko" ? "활동 기록이 없습니다" : "No activity yet"}
          </EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "`oxipage cache refresh --extension activity` 로 동기화하세요."
              : "Sync with `oxipage cache refresh --extension activity`."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "활동" : "Activity"}</PageTitle>
      <ul className="space-y-3">
        {events.map((e) => (
          <li
            key={e.id}
            className="flex items-start gap-3 rounded-lg border border-line bg-surface p-4"
          >
            <div className="flex size-9 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
              <Activity className="size-4" />
            </div>
            <div className="min-w-0 flex-1 space-y-1">
              <p className="text-sm text-foreground">{e.summary}</p>
              <div className="flex flex-wrap items-center gap-2 text-xs text-subtle">
                <Badge variant="secondary">{e.event_type}</Badge>
                <span>{e.repo_full_name}</span>
                <span>· {relativeTime(e.occurred_at, lang)}</span>
              </div>
            </div>
            <a
              href={e.url}
              target="_blank"
              rel="noopener noreferrer"
              className="text-subtle hover:text-primary"
              aria-label="open"
            >
              <ExternalLink className="size-4" />
            </a>
          </li>
        ))}
      </ul>
    </article>
  );
}
