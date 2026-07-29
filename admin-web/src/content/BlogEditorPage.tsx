// Blog editor with split-pane markdown editor

import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router";
import MDEditor from "@uiw/react-md-editor";
import { useSite, getBlogPost, createBlogPost, updateBlogPost, publishBlogPost } from "../shared/api";
import { Button } from "../shared/ui/button";
import { Input } from "../shared/ui/input";

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
        const res = await getBlogPost(slug);
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
        const res = await createBlogPost({ title, body, lang, tags, slug: undefined });
        nav(`/content/blog/${res.data.slug}`, { replace: true });
      } else {
        await updateBlogPost(slug!, { title, body, lang, tags });
        if (!draft) {
          await publishBlogPost(slug!);
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
    return <div><h1 className="text-lg font-semibold mb-2">글 편집</h1><p className="text-sm text-muted">사이트를 선택하세요.</p></div>;
  }

  if (fetching) {
    return <div><h1 className="text-lg font-semibold mb-2">로딩 중...</h1></div>;
  }

  return (
    <div>
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <h1 className="text-lg font-semibold">{isNew ? "새 글" : "글 편집"}</h1>
          <p className="text-xs text-muted">{activeSite.name}</p>
        </div>
        <div className="flex gap-2">
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
        <div className="text-sm text-destructive mb-4 bg-destructive/10 p-3 rounded border border-destructive/20">{error}</div>
      )}

      {/* Title */}
      <Input className="mb-3 text-base font-semibold" placeholder="제목" value={title} onChange={(e) => setTitle(e.target.value)} />

      {/* Meta row */}
      <div className="flex items-center gap-3 mb-3">
        <select
          value={lang}
          onChange={(e) => setLang(e.target.value)}
          className="h-9 rounded-md border border-line bg-canvas px-2 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <option value="ko">한국어</option>
          <option value="en">English</option>
        </select>

        <div className="flex items-center gap-1 flex-1">
          <Input className="text-xs" placeholder="태그 추가..." value={tagInput} onChange={(e) => setTagInput(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addTag(); } }} />
          <button onClick={addTag} className="h-9 w-9 shrink-0 rounded-md border border-line bg-surface text-foreground hover:bg-raised text-xs cursor-pointer">+</button>
        </div>
      </div>

      {tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-3">
          {tags.map((t) => (
            <span key={t} className="inline-flex items-center gap-1 rounded bg-surface px-2 py-0.5 text-xs text-muted">
              {t}
              <button onClick={() => removeTag(t)} className="text-subtle hover:text-foreground cursor-pointer">×</button>
            </span>
          ))}
        </div>
      )}

      {/* Markdown editor — split pane */}
      <div data-color-mode={document.documentElement.dataset.theme ?? "light"}>
        <MDEditor
          value={body}
          onChange={(val) => setBody(val || "")}
          height={500}
          preview="live"
          className="rounded-md overflow-hidden"
        />
      </div>
    </div>
  );
}
