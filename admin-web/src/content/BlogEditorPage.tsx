// Blog editor with split-pane markdown editor

import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router";
import MDEditor from "@uiw/react-md-editor";
import { useSite, getBlogPost, createBlogPost, updateBlogPost, publishBlogPost } from "../shared/api";
import { Button } from "../shared/ui/button";

export function BlogEditorPage() {
  const { slug } = useParams();
  const { activeSite } = useSite();
  const nav = useNavigate();
  const isNew = !slug;

  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [lang, setLang] = useState("ko");
  const [tags, setTags] = useState<string[]>([]);
  const [tagInput, setTagInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [fetching, setFetching] = useState(!isNew);
  const [error, setError] = useState<string | null>(null);

  // Load existing post
  useEffect(() => {
    if (!slug || !activeSite) return;
    (async () => {
      try {
        const res = await getBlogPost(activeSite.name, slug);
        setTitle(res.data.title);
        setBody(res.data.body);
        setLang(res.data.lang);
        setTags(res.data.tags);
      } catch (e: any) {
        setError(e.message);
      } finally {
        setFetching(false);
      }
    })();
  }, [slug, activeSite?.name]);

  const addTag = () => {
    const t = tagInput.trim();
    if (t && !tags.includes(t)) setTags([...tags, t]);
    setTagInput("");
  };

  const removeTag = (t: string) => setTags(tags.filter((x) => x !== t));

  const save = async (draft: boolean) => {
    if (!activeSite) return;
    if (!title.trim()) { setError("제목을 입력하세요."); return; }
    setLoading(true);
    setError(null);
    try {
      if (isNew) {
        const res = await createBlogPost(activeSite.name, { title, body, lang, tags, slug: undefined });
        nav(`/content/blog/${res.data.slug}`, { replace: true });
      } else {
        await updateBlogPost(activeSite.name, slug!, { title, body, lang, tags });
        if (!draft) {
          await publishBlogPost(activeSite.name, slug!);
        }
        nav("/content/blog");
      }
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  if (!activeSite) {
    return <div><h1 className="text-lg font-semibold mb-2">글 편집</h1><p className="text-sm text-[#777]">사이트를 선택하세요.</p></div>;
  }

  if (fetching) {
    return <div><h1 className="text-lg font-semibold mb-2">로딩 중...</h1></div>;
  }

  return (
    <div>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <div>
          <h1 className="text-lg font-semibold">{isNew ? "새 글" : "글 편집"}</h1>
          <p className="text-xs text-[#777]">{activeSite.name}</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <Button variant="outline" onClick={() => nav("/content/blog")}>목록</Button>
          <Button variant="secondary" onClick={() => save(true)} disabled={loading}>
            {loading ? "저장 중..." : "초안 저장"}
          </Button>
          <Button onClick={() => save(false)} disabled={loading}>
            {loading ? "저장 중..." : (isNew ? "작성 + 발행" : "저장 + 발행")}
          </Button>
        </div>
      </div>

      {error && (
        <div className="text-sm text-red-600 mb-4 bg-red-50 p-3 rounded border border-red-200">{error}</div>
      )}

      {/* Title */}
      <input
        type="text"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        placeholder="제목"
        style={{
          width: "100%",
          padding: "10px 12px",
          fontSize: 16,
          fontWeight: 600,
          border: "1px solid #e8e4e0",
          borderRadius: 6,
          marginBottom: 12,
          outline: "none",
          boxSizing: "border-box",
        }}
      />

      {/* Meta row */}
      <div style={{ display: "flex", gap: 12, alignItems: "center", marginBottom: 12 }}>
        <select
          value={lang}
          onChange={(e) => setLang(e.target.value)}
          style={{ padding: "6px 8px", border: "1px solid #e8e4e0", borderRadius: 4, fontSize: 13 }}
        >
          <option value="ko">한국어</option>
          <option value="en">English</option>
        </select>

        <div style={{ display: "flex", alignItems: "center", gap: 4, flex: 1 }}>
          <input
            type="text"
            value={tagInput}
            onChange={(e) => setTagInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addTag(); } }}
            placeholder="태그 추가..."
            style={{
              padding: "6px 8px",
              border: "1px solid #e8e4e0",
              borderRadius: 4,
              fontSize: 13,
              flex: 1,
              minWidth: 100,
              outline: "none",
            }}
          />
          <button onClick={addTag} style={{ padding: "6px 10px", border: "1px solid #e8e4e0", borderRadius: 4, background: "#fff", fontSize: 12, cursor: "pointer" }}>+</button>
        </div>
      </div>

      {tags.length > 0 && (
        <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginBottom: 12 }}>
          {tags.map((t) => (
            <span key={t} style={{ display: "inline-flex", alignItems: "center", gap: 4, padding: "2px 8px", background: "#f5f2ed", borderRadius: 4, fontSize: 12 }}>
              {t}
              <button onClick={() => removeTag(t)} style={{ background: "none", border: "none", cursor: "pointer", color: "#888", fontSize: 14, padding: 0 }}>×</button>
            </span>
          ))}
        </div>
      )}

      {/* Markdown editor — split pane */}
      <div data-color-mode="light">
        <MDEditor
          value={body}
          onChange={(val) => setBody(val || "")}
          height={500}
          preview="live"
          style={{ borderRadius: 6, overflow: "hidden" }}
        />
      </div>
    </div>
  );
}
