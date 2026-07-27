import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, Calendar } from "lucide-react";
import { Link, useParams } from "react-router";

import { fetchBlogPost } from "../../shared/api";
import { useLanguage } from "../../shared/language";
import { Markdown } from "../../shared/Markdown";
import { Badge } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";
import { Card, CardContent } from "../../shared/ui/card";

export function BlogPostPage() {
  const { slug = "" } = useParams();
  const { lang } = useLanguage();
  const { data: post, isLoading, error } = useQuery({
    queryKey: ["blog", slug],
    queryFn: () => fetchBlogPost(slug),
    enabled: !!slug,
  });

  if (isLoading) return <p className="text-subtle">…</p>;
  if (error || !post) {
    return (
      <div className="space-y-4">
        <Button variant="ghost" size="sm" asChild>
          <Link to="/blog">
            <ArrowLeft />
            {lang === "ko" ? "블로그" : "Blog"}
          </Link>
        </Button>
        <p className="text-subtle">
          {lang === "ko" ? "게시물을 찾을 수 없습니다." : "Post not found."}
        </p>
      </div>
    );
  }

  return (
    <article className="space-y-6">
      <Button variant="ghost" size="sm" asChild className="-ml-2">
        <Link to="/blog">
          <ArrowLeft />
          {lang === "ko" ? "블로그" : "Blog"}
        </Link>
      </Button>

      <header className="space-y-3">
        <h1 className="font-serif text-3xl font-semibold tracking-tight text-foreground">
          {post.title}
        </h1>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-sm text-subtle">
          <span className="inline-flex items-center gap-1">
            <Calendar className="size-3.5" />
            <time>{(post.published_at ?? post.created_at).slice(0, 10)}</time>
          </span>
          <Badge variant="outline">{post.lang === "ko" ? "한국어" : "English"}</Badge>
          {post.tags.length > 0 &&
            post.tags.map((t) => (
              <Badge key={t} variant="secondary">
                {t}
              </Badge>
            ))}
        </div>
      </header>

      <Card>
        <CardContent className="markdown pt-6">
          <Markdown
            source={post.body || `*${lang === "ko" ? "내용 없음" : "No content"}*`}
          />
        </CardContent>
      </Card>
    </article>
  );
}
