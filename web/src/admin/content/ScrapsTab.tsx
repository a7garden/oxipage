import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { contentClient } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Badge } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Textarea } from "../../shared/ui/textarea";
import { Drawer, DrawerField } from "../../shared/ui/drawer";
import { Pencil, Trash2, Send, Plus } from "lucide-react";
import { useRowFilter } from "../shared/useRowFilter";
import { field, str } from "../shared/row-utils";

interface ScrapItem {
  id: number;
  source: string;
  source_item_id: string | null;
  source_url: string;
  title: string;
  og_image_url: string | null;
  note_ko: string | null;
  note_en: string | null;
  tags: string[];
  scraped_at: string;
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

interface FormState {
  source_url: string;
  title: string;
  note_ko: string;
  note_en: string;
  tags: string;
}

const EMPTY: FormState = { source_url: "", title: "", note_ko: "", note_en: "", tags: "" };

export function ScrapsTab({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<null | ScrapItem | "new">(null);
  const [form, setForm] = useState<FormState>(EMPTY);
  const [error, setError] = useState<string | null>(null);

  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "scraps"],
    queryFn: async () => {
      const [published, queue] = await Promise.all([
        contentClient.list<ScrapItem>(slug, "scraps"),
        contentClient.list<ScrapItem>(slug, "scraps/queue"),
      ]);
      return [...queue, ...published];
    },
  });

  const [search, setSearch] = useState("");
  const filtered = useRowFilter(data ?? [], search, (row) => [row.title, row.source_url]);

  const save = useMutation({
    mutationFn: async () => {
      const payload = {
        source_url: form.source_url.trim(),
        title: form.title.trim(),
        note_ko: form.note_ko || null,
        note_en: form.note_en || null,
        tags: form.tags.split(",").map((t) => t.trim()).filter(Boolean),
      };
      if (editing === "new") return contentClient.create<ScrapItem>(slug, "scraps", payload);
      if (editing) return contentClient.update<ScrapItem>(slug, "scraps", String(editing.id), payload);
      throw new Error("no row");
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["site", slug, "content", "scraps"] });
      setEditing(null);
      setForm(EMPTY);
      setError(null);
    },
    onError: (e) => setError(e instanceof Error ? e.message : "Save failed"),
  });

  const publish = useMutation({
    mutationFn: (id: number) => contentClient.action<ScrapItem>(slug, "scraps", String(id), "publish"),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "scraps"] }),
  });

  const remove = useMutation({
    mutationFn: (id: number) => contentClient.delete(slug, "scraps", String(id)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "scraps"] }),
  });

  const openEdit = (s: ScrapItem) => {
    setEditing(s);
    setForm({
      source_url: s.source_url,
      title: s.title,
      note_ko: s.note_ko ?? "",
      note_en: s.note_en ?? "",
      tags: (s.tags ?? []).join(", "),
    });
    setError(null);
  };

  const columns = [
    {
      key: "title", label: "Title",
      render: (row: unknown) => {
        const r = row as ScrapItem;
        return (
          <div>
            <div className="font-medium">{r.title}</div>
            <a href={r.source_url} target="_blank" rel="noreferrer" className="text-xs text-muted hover:text-foreground truncate block max-w-[400px]">
              {r.source_url}
            </a>
          </div>
        );
      },
    },
    {
      key: "source", label: "Source", width: "80px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "source"))}</span>,
    },
    {
      key: "status", label: "Status", width: "96px" as const,
      render: (row: unknown) => (
        <Badge variant={field(row, "published_at") ? "positive" : "secondary"}>
          {field(row, "published_at") ? "Published" : "Queued"}
        </Badge>
      ),
    },
    {
      key: "collected", label: "Collected", width: "160px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "created_at"))}</span>,
    },
    {
      key: "actions", label: "Actions", width: "140px" as const,
      render: (row: unknown) => {
        const r = row as ScrapItem;
        return (
          <div className="flex justify-end gap-1">
            <button
              onClick={() => publish.mutate(r.id)}
              disabled={publish.isPending || !!r.published_at}
              title={r.published_at ? "Already published" : "Publish"}
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
              onClick={() => { if (confirm(`Delete "${r.title}"?`)) remove.mutate(r.id); }}
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
        <Input placeholder="Search scraps..." className="w-60" value={search} onChange={(e) => setSearch(e.target.value)} />
        <Button size="sm" onClick={() => { setEditing("new"); setForm(EMPTY); setError(null); }}>
          <Plus size={14} className="mr-1" /> Add Scrap
        </Button>
      </div>
      <ContentTable
        columns={columns}
        data={filtered}
        isLoading={isLoading}
        emptyTitle="No scraps yet"
        emptyDescription="Collect your first scrap."
      />

      <Drawer
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing === "new" ? "Add Scrap" : "Edit Scrap"}
        width="w-[560px]"
        footer={
          <>
            <Button variant="outline" onClick={() => setEditing(null)} disabled={save.isPending}>Cancel</Button>
            <Button onClick={() => save.mutate()} disabled={save.isPending || !form.title.trim() || !form.source_url.trim()}>
              {save.isPending ? "Saving..." : "Save"}
            </Button>
          </>
        }
      >
        <DrawerField label="Title" required>
          <Input value={form.title} onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))} autoFocus />
        </DrawerField>
        <DrawerField label="Source URL" required>
          <Input value={form.source_url} onChange={(e) => setForm((f) => ({ ...f, source_url: e.target.value }))} placeholder="https://..." type="url" />
        </DrawerField>
        <DrawerField label="Tags" hint="Comma-separated">
          <Input value={form.tags} onChange={(e) => setForm((f) => ({ ...f, tags: e.target.value }))} />
        </DrawerField>
        <DrawerField label="Note (Korean)">
          <Textarea value={form.note_ko} onChange={(e) => setForm((f) => ({ ...f, note_ko: e.target.value }))} rows={5} />
        </DrawerField>
        <DrawerField label="Note (English)">
          <Textarea value={form.note_en} onChange={(e) => setForm((f) => ({ ...f, note_en: e.target.value }))} rows={5} />
        </DrawerField>
        {error && <p className="text-sm text-red-600">{error}</p>}
      </Drawer>
    </div>
  );
}
