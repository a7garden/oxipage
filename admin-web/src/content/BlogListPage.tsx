// Blog list — draft/published filter, create, edit, publish, delete

import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { useSite, listBlogPosts, deleteBlogPost, publishBlogPost, type BlogPost } from "../shared/api";
import { Card } from "../shared/ui/card";
import { Button } from "../shared/ui/button";
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
    return <div><h1 className="text-lg font-semibold mb-2">블로그</h1><p className="text-sm text-[#777]">사이트를 선택하세요.</p></div>;
  }

  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <div>
          <h1 className="text-lg font-semibold">블로그</h1>
          <p className="text-xs text-[#777]">{activeSite.name}</p>
        </div>
        <Button onClick={() => nav("/content/blog/new")}>새 글</Button>
      </div>

      {/* Filter tabs */}
      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        {[
          { label: "전체", value: undefined },
          { label: "초안", value: true },
          { label: "발행됨", value: false },
        ].map((f) => (
          <button
            key={String(f.value)}
            onClick={() => setDraftFilter(f.value)}
            style={{
              padding: "4px 12px",
              borderRadius: 6,
              fontSize: 13,
              border: "1px solid #e8e4e0",
              background: draftFilter === f.value ? "oklch(50% 0.14 160)" : "#fff",
              color: draftFilter === f.value ? "#fff" : "#555",
              cursor: "pointer",
            }}
          >
            {f.label}
          </button>
        ))}
      </div>

      {loading ? (
        <p className="text-sm text-[#777]">로딩 중...</p>
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
                      style={{ background: "none", border: "none", cursor: "pointer", color: "inherit", font: "inherit", textAlign: "left" }}
                    >
                      {post.title}
                    </button>
                  </td>
                  <td className="text-xs text-[#777]">{post.lang}</td>
                  <td>
                    <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                      {post.tags.map((t) => (
                        <span key={t} className="text-xs text-[#777] bg-[#f5f2ed] px-2 py-0.5 rounded">{t}</span>
                      ))}
                    </div>
                  </td>
                  <td>
                    <Badge variant={post.published_at ? "active" : "inactive"}>
                      {post.published_at ? "발행" : "초안"}
                    </Badge>
                  </td>
                  <td className="text-xs text-[#777]">{post.updated_at?.slice(0, 10)}</td>
                  <td>
                    <div style={{ display: "flex", gap: 4 }}>
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
                  <td colSpan={6} className="text-center text-sm text-[#777] py-8">게시물이 없습니다.</td>
                </tr>
              )}
            </tbody>
          </table>
        </Card>
      )}
    </div>
  );
}
