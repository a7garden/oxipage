// Blog list — draft/published filter, create, edit, publish, delete

import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { useSite, listBlogPosts, deleteBlogPost, publishBlogPost, type BlogPost } from "../shared/api";
import { Card } from "../shared/ui/card";
import { Button } from "../shared/ui/button";
import { cn } from "../shared/ui/cn";
import { Badge } from "../shared/ui/badge";

export function BlogListPage() {
  const { activeSite } = useSite();
  const nav = useNavigate();
  const [posts, setPosts] = useState<BlogPost[]>([]);
  const [draftFilter, setDraftFilter] = useState<boolean | undefined>(undefined);
  const [loading, setLoading] = useState(true);

  const fetchPosts = async () => {
    if (!activeSite) return;
    setLoading(true);
    try {
      const res = await listBlogPosts(draftFilter);
      setPosts(res.data);
    } catch {
      setPosts([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchPosts();
  }, [activeSite?.name, draftFilter]);

  const handleDelete = async (slug: string) => {
    if (!activeSite || !confirm(`"${slug}"를 삭제하시겠습니까?`)) return;
    try {
      await deleteBlogPost(slug);
      fetchPosts();
    } catch (e: any) {
      alert("Delete failed: " + e.message);
    }
  };

  const handlePublish = async (slug: string) => {
    if (!activeSite) return;
    try {
      await publishBlogPost(slug);
      fetchPosts();
    } catch (e: any) {
      alert("Publish failed: " + e.message);
    }
  };

  if (!activeSite) {
    return <div><h1 className="text-lg font-semibold mb-2">블로그</h1><p className="text-sm text-muted">사이트를 선택하세요.</p></div>;
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <div>
          <h1 className="text-lg font-semibold">블로그</h1>
          <p className="text-xs text-muted">{activeSite.name}</p>
        </div>
        <Button onClick={() => nav("/content/blog/new")}>새 글</Button>
      </div>

      {/* Filter tabs */}
      <div className="flex gap-2 mb-4">
        {[
          { label: "전체", value: undefined },
          { label: "초안", value: true },
          { label: "발행됨", value: false },
        ].map((f) => (
          <button
            key={String(f.value)}
            onClick={() => setDraftFilter(f.value)}
            className={cn("h-8 rounded-md border border-line px-3 text-xs cursor-pointer transition-colors", draftFilter === f.value ? "bg-primary text-primary-foreground" : "bg-canvas text-muted hover:bg-surface")}
          >
            {f.label}
          </button>
        ))}
      </div>

      {loading ? (
        <p className="text-sm text-muted">로딩 중...</p>
      ) : (
        <Card>
          <table className="admin-table">
            <thead>
              <tr>
                <th>제목</th>
                <th>언어</th>
                <th>태그</th>
                <th>상태</th>
                <th>수정일</th>
                <th>액션</th>
              </tr>
            </thead>
            <tbody>
              {posts.map((post) => (
                <tr key={post.slug}>
                  <td className="font-medium">
                    <button
                      onClick={() => nav(`/content/blog/${post.slug}`)}
                      className="text-left cursor-pointer text-foreground hover:text-primary transition-colors"
                    >
                      {post.title}
                    </button>
                  </td>
                  <td className="text-xs text-muted">{post.lang}</td>
                  <td>
                    <div className="flex flex-wrap gap-1">
                      {post.tags.map((t) => (
                        <span key={t} className="text-xs text-muted bg-surface px-2 py-0.5 rounded">{t}</span>
                      ))}
                    </div>
                  </td>
                  <td>
                    <Badge variant={post.published_at ? "active" : "inactive"}>
                      {post.published_at ? "발행" : "초안"}
                    </Badge>
                  </td>
                  <td className="text-xs text-muted">{post.updated_at?.slice(0, 10)}</td>
                  <td>
                    <div className="flex gap-1">
                      <Button size="sm" variant="outline" onClick={() => nav(`/content/blog/${post.slug}`)}>편집</Button>
                      {!post.published_at && (
                        <Button size="sm" variant="primary" onClick={() => handlePublish(post.slug)}>발행</Button>
                      )}
                      <Button size="sm" variant="destructive" onClick={() => handleDelete(post.slug)}>삭제</Button>
                    </div>
                  </td>
                </tr>
              ))}
              {posts.length === 0 && (
                <tr>
                  <td colSpan={6} className="text-center text-sm text-muted py-8">게시물이 없습니다.</td>
                </tr>
              )}
            </tbody>
          </table>
        </Card>
      )}
    </div>
  );
}
