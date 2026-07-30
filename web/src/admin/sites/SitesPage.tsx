import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router";
import { listSites, removeSite, setDefaultSite } from "../shared/api";
import { PageHeader, PageTitle, PageActions } from "../../shared/ui/page-header";
import { Button } from "../../shared/ui/button";
import { Card, CardContent } from "../../shared/ui/card";
import { Badge } from "../../shared/ui/badge";
import { EmptyState, EmptyStateIcon, EmptyStateTitle, EmptyStateDescription } from "../../shared/ui/empty-state";
import { Skeleton } from "../../shared/ui/skeleton";

export function SitesPage() {
  const qc = useQueryClient();
  const { data, isLoading } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  const remove = useMutation({
    mutationFn: removeSite,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["sites"] }),
  });
  const setDef = useMutation({
    mutationFn: setDefaultSite,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["sites"] }),
  });

  const sites = data?.data ?? [];

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-24 w-full" />
        <Skeleton className="h-24 w-full" />
      </div>
    );
  }

  if (sites.length === 0) {
    return (
      <EmptyState>
        <EmptyStateIcon>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z" />
          </svg>
        </EmptyStateIcon>
        <EmptyStateTitle>등록된 사이트가 없습니다</EmptyStateTitle>
        <EmptyStateDescription>
          첫 사이트를 추가하면 여기에 표시됩니다. 사이트를 만들면 블로그, 프로젝트, 영화 기록 등
          콘텐츠를 관리할 수 있습니다.
        </EmptyStateDescription>
        <Button asChild>
          <Link to="/sites/new">새 사이트 추가</Link>
        </Button>
      </EmptyState>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader>
        <PageTitle>사이트 관리</PageTitle>
        <PageActions>
          <Button asChild>
            <Link to="/sites/new">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M5 12h14" />
                <path d="M12 5v14" />
              </svg>
              새 사이트 추가
            </Link>
          </Button>
        </PageActions>
      </PageHeader>

      <div className="space-y-3">
        {sites.map((s) => (
          <Card key={s.name}>
            <CardContent className="flex items-center gap-4 py-4">
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <Link
                    to={`/s/${s.name}`}
                    className="font-medium text-foreground hover:text-primary transition-colors"
                  >
                    {s.name}
                  </Link>
                  {s.active && (
                    <Badge variant="positive" className="shrink-0">
                      활성
                    </Badge>
                  )}
                </div>
                <p className="text-sm text-muted font-mono truncate">{s.path}</p>
              </div>

              <div className="flex items-center gap-2 shrink-0">
                {!s.active && (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => setDef.mutate(s.name)}
                    disabled={setDef.isPending}
                  >
                    기본으로 설정
                  </Button>
                )}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => remove.mutate(s.name)}
                  disabled={remove.isPending}
                  className="text-destructive hover:bg-destructive/10"
                >
                  삭제
                </Button>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
