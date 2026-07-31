import { Calendar } from "lucide-react";

import { Markdown } from "../../shared/Markdown";
import { Badge } from "../../shared/ui/badge";
import { Card, CardContent } from "../../shared/ui/card";

export interface BlogPostData {
  title: string;
  body: string;
  lang: "ko" | "en";
  tags: string[];
  published_at: string | null;
  created_at: string;
}

interface BlogPostViewProps {
  post: BlogPostData;
  language: "ko" | "en";
}

export function BlogPostView({ post, language: _language }: BlogPostViewProps) {
  const date = (post.published_at ?? post.created_at).slice(0, 10);
  return (
    <article className="space-y-6">
      <header className="space-y-3">
        <h1 className="font-serif text-3xl font-semibold tracking-tight text-foreground">
          {post.title}
        </h1>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-sm text-subtle">
          <span className="inline-flex items-center gap-1">
            <Calendar className="size-3.5" />
            <time>{date}</time>
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
          <Markdown source={post.body || "*No content*"} />
        </CardContent>
      </Card>
    </article>
  );
}