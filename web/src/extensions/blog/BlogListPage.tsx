import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";

import { fetchBlogPosts } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Badge } from "../../shared/ui/badge";
import { Card } from "../../shared/ui/card";
import { EmptyState, EmptyStateDescription, EmptyStateIcon, EmptyStateTitle } from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { NotebookPen } from "lucide-react";

export function BlogListPage() {
  const { lang } = useLanguage();
  const { data: posts, isLoading } = useQuery({
    queryKey: ["blog", "list", lang],
    queryFn: () => fetchBlogPosts(),
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (!posts || posts.length === 0) {
    return (
      <div className="space-y-6">
        <PageTitle>{lang === "ko" ? "블로그" : "Blog"}</PageTitle>
        <EmptyState>
          <EmptyStateIcon>
            <NotebookPen className="size-5" />
          </EmptyStateIcon>
          <EmptyStateTitle>
            {lang === "ko" ? "아직 게시물이 없습니다" : "No posts yet"}
          </EmptyStateTitle>
          <EmptyStateDescription>
            {lang === "ko"
              ? "첫 글이 곧 올라옵니다."
              : "The first post is on its way."}
          </EmptyStateDescription>
        </EmptyState>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <PageTitle>{lang === "ko" ? "블로그" : "Blog"}</PageTitle>
      <ul className="space-y-3">
        {posts.map((p) => (
          <li key={p.slug}>
            <Card className="transition-[border-color,box-shadow] duration-200 hover:border-primary/40 hover:shadow-md">
              <Link to={`/blog/${p.slug}`} className="block p-5 text-foreground no-underline">
                <h2 className="font-serif text-xl font-semibold tracking-tight">
                  {p.title}
                </h2>
                <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-subtle">
                  <time>{(p.published_at ?? p.created_at).slice(0, 10)}</time>
                  {p.tags.length > 0 &&
                    p.tags.map((t) => (
                      <Badge key={t} variant="secondary">
                        {t}
                      </Badge>
                    ))}
                </div>
              </Link>
            </Card>
          </li>
        ))}
      </ul>
    </article>
  );
}
