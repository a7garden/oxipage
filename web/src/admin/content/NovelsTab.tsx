import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { contentClient } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Button } from "../../shared/ui/button";
import { Badge } from "../../shared/ui/badge";
import { Input } from "../../shared/ui/input";
import { Textarea } from "../../shared/ui/textarea";
import { MarkdownEditor } from "../shared/ui/MarkdownEditor";
import { Drawer, DrawerField } from "../../shared/ui/drawer";
import { Pencil, Trash2, Send, Plus, ChevronDown, ChevronUp } from "lucide-react";
import { useRowFilter } from "../shared/useRowFilter";
import {
  listChapters, createChapter, updateChapter, deleteChapter, publishChapter,
  type NovelChapter,
} from "../shared/api";
import { field, str } from "../shared/row-utils";

interface Novel {
  id: number;
  slug: string;
  title: string;
  synopsis: string | null;
  cover_image: string | null;
  status: string;
  tags: string[];
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

interface FormState {
  title: string;
  synopsis: string;
  cover_image: string;
  status: string;
  tags: string;
}

const EMPTY: FormState = {
  title: "",
  synopsis: "",
  cover_image: "",
  status: "ongoing",
  tags: "",
};

export function NovelsTab({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<null | Novel | "new">(null);
  const [form, setForm] = useState<FormState>(EMPTY);
  const [error, setError] = useState<string | null>(null);
  const [showChapters, setShowChapters] = useState(false);
  const [editingChapter, setEditingChapter] = useState<null | number | "new">(null);
  const [chapterForm, setChapterForm] = useState({ chapter_order: 1, title: "", body: "" });

  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "novels"],
    queryFn: () => contentClient.list<Novel>(slug, "novels", { draft: true }),
  });

  const [search, setSearch] = useState("");
  const filtered = useRowFilter(data ?? [], search, (row) => [row.title, row.slug]);

  const save = useMutation({
    mutationFn: async () => {
      const payload = {
        title: form.title.trim(),
        synopsis: form.synopsis || null,
        cover_image: form.cover_image || null,
        status: form.status,
        tags: form.tags.split(",").map((t) => t.trim()).filter(Boolean),
      };
      if (editing === "new") return contentClient.create<Novel>(slug, "novels", payload);
      if (editing) return contentClient.update<Novel>(slug, "novels", editing.slug, payload);
      throw new Error("no row");
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["site", slug, "content", "novels"] });
      setEditing(null);
      setForm(EMPTY);
      setError(null);
    },
    onError: (e) => setError(e instanceof Error ? e.message : "Save failed"),
  });

  const publish = useMutation({
    mutationFn: (s: string) => contentClient.action<Novel>(slug, "novels", s, "publish"),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "novels"] }),
  });

  const remove = useMutation({
    mutationFn: (s: string) => contentClient.delete(slug, "novels", s),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "novels"] }),
  });

  const novelSlug = editing && editing !== "new" ? editing.slug : undefined;
  const { data: chapters = [] } = useQuery({
    queryKey: ["site", slug, "novels", novelSlug, "chapters"],
    queryFn: () => listChapters(slug!, novelSlug!),
    enabled: !!novelSlug && showChapters,
  });

  const createChapterMut = useMutation({
    mutationFn: (input: { chapter_order: number; title: string; body: string }) =>
      createChapter(slug!, novelSlug!, input),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ["site", slug, "novels", novelSlug, "chapters"] }); setEditingChapter(null); setChapterForm({ chapter_order: 1, title: "", body: "" }); },
  });

  const updateChapterMut = useMutation({
    mutationFn: ({ order, patch }: { order: number; patch: { title?: string; body?: string; chapter_order?: number } }) =>
      updateChapter(slug!, novelSlug!, order, patch),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "novels", novelSlug, "chapters"] }),
  });

  const deleteChapterMut = useMutation({
    mutationFn: (order: number) => deleteChapter(slug!, novelSlug!, order),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "novels", novelSlug, "chapters"] }),
  });

  const publishChapterMut = useMutation({
    mutationFn: (order: number) => publishChapter(slug!, novelSlug!, order),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "novels", novelSlug, "chapters"] }),
  });

  const handleReorder = (order: number, direction: -1 | 1) => {
    const idx = chapters.findIndex((c) => c.chapter_order === order);
    if (idx === -1) return;
    const target = chapters[idx + direction];
    if (!target) return;
    updateChapterMut.mutate({ order, patch: { chapter_order: target.chapter_order } });
    updateChapterMut.mutate({ order: target.chapter_order, patch: { chapter_order: order } });
  };

  const openEdit = (n: Novel) => {
    setEditing(n);
    setForm({
      title: n.title,
      synopsis: n.synopsis ?? "",
      cover_image: n.cover_image ?? "",
      status: n.status,
      tags: (n.tags ?? []).join(", "),
    });
    setError(null);
  };

  const columns = [
    {
      key: "title", label: "Title",
      render: (row: unknown) => {
        const r = row as Novel;
        return (
          <div>
            <div className="font-medium">{r.title}</div>
            <div className="text-xs text-muted">/{r.slug}</div>
          </div>
        );
      },
    },
    {
      key: "status", label: "Status", width: "100px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "status"))}</span>,
    },
    {
      key: "tags", label: "Tags", width: "180px" as const,
      render: (row: unknown) => {
        const tags = field(row, "tags");
        return <span className="text-xs text-muted">{Array.isArray(tags) ? tags.join(", ") : "—"}</span>;
      },
    },
    {
      key: "updated", label: "Updated", width: "160px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "updated_at"))}</span>,
    },
    {
      key: "actions", label: "Actions", width: "140px" as const,
      render: (row: unknown) => {
        const r = row as Novel;
        return (
          <div className="flex justify-end gap-1">
            <button
              onClick={() => publish.mutate(r.slug)}
              disabled={publish.isPending || !!r.published_at}
              className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-[#22c55e] hover:bg-surface/50 disabled:opacity-30 disabled:hover:text-muted"
            >
              <Send size={14} />
            </button>
            <button
              onClick={() => openEdit(r)}
              className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-foreground hover:bg-surface/50"
              aria-label="Edit"
            >
              <Pencil size={14} />
            </button>
            <button
              onClick={() => { if (confirm(`Delete "${r.title}"?`)) remove.mutate(r.slug); }}
              className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-red-600 hover:bg-red-50"
              aria-label="Delete"
            >
              <Trash2 size={14} />
            </button>
          </div>
        );
      },
    },
  ];

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <Input placeholder="Search novels..." className="w-60" value={search} onChange={(e) => setSearch(e.target.value)} />
        <Button size="sm" onClick={() => { setEditing("new"); setForm(EMPTY); setError(null); }}>
          <Plus size={14} className="mr-1" /> New Novel
        </Button>
      </div>
      <ContentTable
        columns={columns}
        data={filtered}
        isLoading={isLoading}
        emptyTitle="No novels yet"
        emptyDescription="Start writing your first novel."
      />

      <Drawer
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing === "new" ? "New Novel" : "Edit Novel"}
        description={editing !== null && editing !== "new" ? `/${editing.slug}` : "Outline a new novel"}
        width="w-[560px]"
        footer={
          <>
            <Button variant="outline" onClick={() => setEditing(null)} disabled={save.isPending}>Cancel</Button>
            <Button onClick={() => save.mutate()} disabled={save.isPending || !form.title.trim()}>
              {save.isPending ? "Saving..." : "Save"}
            </Button>
          </>
        }
      >
        <DrawerField label="Title" required>
          <Input value={form.title} onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))} autoFocus />
        </DrawerField>
        <DrawerField label="Status">
          <select
            value={form.status}
            onChange={(e) => setForm((f) => ({ ...f, status: e.target.value }))}
            className="h-10 w-full rounded-md border border-line bg-canvas px-3 text-sm text-foreground"
          >
            <option value="ongoing">ongoing</option>
            <option value="completed">completed</option>
            <option value="hiatus">hiatus</option>
          </select>
        </DrawerField>
        <DrawerField label="Cover image URL">
          <Input value={form.cover_image} onChange={(e) => setForm((f) => ({ ...f, cover_image: e.target.value }))} placeholder="https://..." />
        </DrawerField>
        <DrawerField label="Tags" hint="Comma-separated">
          <Input value={form.tags} onChange={(e) => setForm((f) => ({ ...f, tags: e.target.value }))} />
        </DrawerField>
        <DrawerField label="Synopsis">
          <Textarea value={form.synopsis} onChange={(e) => setForm((f) => ({ ...f, synopsis: e.target.value }))} rows={8} />
        </DrawerField>

        {novelSlug && (
          <div className="border-t border-line pt-4 mt-4">
            <button
              onClick={() => setShowChapters(!showChapters)}
              className="flex items-center gap-2 text-sm font-semibold text-foreground mb-3 w-full text-left"
            >
              <ChevronDown size={14} className={`transition-transform ${showChapters ? "" : "-rotate-90"}`} />
              Chapters ({chapters.length})
            </button>
            {showChapters && (
              <div className="space-y-1 mb-3">
                {chapters.map((ch, i) => (
                  <div key={ch.id} className="flex items-center gap-2 px-2 py-1.5 rounded border border-line text-sm">
                    <span className="text-muted w-6 shrink-0 text-center">{ch.chapter_order}</span>
                    <span className="flex-1 truncate">{ch.title}</span>
                    <Badge variant={ch.published_at ? "default" : "outline"} className="text-[10px] px-1.5">
                      {ch.published_at ? "Published" : "Draft"}
                    </Badge>
                    <button onClick={() => { setEditingChapter(ch.chapter_order); setChapterForm({ chapter_order: ch.chapter_order, title: ch.title, body: ch.body }); }} className="text-muted hover:text-foreground"><Pencil size={14} /></button>
                    <button onClick={() => handleReorder(ch.chapter_order, -1)} disabled={i === 0} className="text-muted hover:text-foreground disabled:opacity-30"><ChevronUp size={14} /></button>
                    <button onClick={() => handleReorder(ch.chapter_order, 1)} disabled={i === chapters.length - 1} className="text-muted hover:text-foreground disabled:opacity-30"><ChevronDown size={14} /></button>
                    <button onClick={() => { if (confirm(`Delete chapter "${ch.title}"?`)) deleteChapterMut.mutate(ch.chapter_order); }} className="text-red-500 hover:text-red-600"><Trash2 size={14} /></button>
                  </div>
                ))}
              </div>
            )}
            <button
              onClick={() => {
                const nextOrder = chapters.length > 0 ? Math.max(...chapters.map((c) => c.chapter_order)) + 1 : 1;
                setEditingChapter("new");
                setChapterForm({ chapter_order: nextOrder, title: "", body: "" });
              }}
              className="flex items-center gap-1 text-xs text-muted hover:text-foreground"
            >
              <Plus size={12} /> Add Chapter
            </button>
            {editingChapter && (
              <div className="border border-line rounded p-3 mt-2 space-y-2 bg-surface/30">
                <Input
                  placeholder="Chapter title"
                  value={chapterForm.title}
                  onChange={(e) => setChapterForm((f) => ({ ...f, title: e.target.value }))}
                />
                <MarkdownEditor value={chapterForm.body} onChange={(v) => setChapterForm((f) => ({ ...f, body: v }))} rows={6} placeholder="Chapter body" />
                <div className="flex gap-2 justify-end">
                  <Button variant="outline" size="sm" onClick={() => setEditingChapter(null)}>Cancel</Button>
                  <Button size="sm" onClick={() => {
                    if (editingChapter === "new") createChapterMut.mutate(chapterForm);
                    else updateChapterMut.mutate({ order: editingChapter, patch: { title: chapterForm.title, body: chapterForm.body } });
                  }}>
                    {editingChapter === "new" ? "Add" : "Save"}
                  </Button>
                </div>
              </div>
            )}
          </div>
        )}

        {error && <p className="text-sm text-red-600">{error}</p>}
      </Drawer>
    </div>
  );
}
