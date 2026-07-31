import { useQuery } from "@tanstack/react-query";

import { fetchBlogPosts } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { EmptyState, EmptyStateDescription, EmptyStateIcon, EmptyStateTitle } from "../../shared/ui/empty-state";
import { PageTitle } from "../../shared/ui/page-header";
import { NotebookPen } from "lucide-react";
import { BlogPostCard } from "./BlogPostCard";

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
          <BlogPostCard
            key={p.slug}
            post={{ slug: p.slug, title: p.title, tags: p.tags, published_at: p.published_at }}
          />
        ))}
      </ul>
    </article>
  );
}
