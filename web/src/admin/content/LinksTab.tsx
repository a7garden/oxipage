import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { contentClient } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Textarea } from "../../shared/ui/textarea";
import { Drawer, DrawerField } from "../../shared/ui/drawer";
import { ImageField } from "../shared/ui/ImageField";
import { TagInput } from "../shared/ui/TagInput";
import { Pencil, Trash2, Plus } from "lucide-react";
import { useRowFilter } from "../shared/useRowFilter";
import { field, str } from "../shared/row-utils";

interface LinkCard {
  id: number;
  title: string;
  url: string;
  description_ko: string | null;
  description_en: string | null;
  thumbnail_url: string | null;
  tags: string[];
  display_order: number;
  featured: boolean;
  created_at: string;
  updated_at: string;
}

interface FormState {
  title: string;
  url: string;
  description_ko: string;
  description_en: string;
  thumbnail_url: string;
  tags: string[];
  display_order: string;
  featured: boolean;
}

const EMPTY: FormState = {
  title: "",
  url: "",
  description_ko: "",
  description_en: "",
  thumbnail_url: "",
  tags: [],
  display_order: "0",
  featured: false,
};

export function LinksTab({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<null | LinkCard | "new">(null);
  const [form, setForm] = useState<FormState>(EMPTY);
  const [error, setError] = useState<string | null>(null);

  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "links"],
    queryFn: () => contentClient.list<LinkCard>(slug, "links"),
  });

  const [search, setSearch] = useState("");
  const filtered = useRowFilter(data ?? [], search, (row) => [row.title, row.url]);

  const save = useMutation({
    mutationFn: async () => {
      const payload = {
        title: form.title.trim(),
        url: form.url.trim(),
        description_ko: form.description_ko || null,
        description_en: form.description_en || null,
        thumbnail_url: form.thumbnail_url || null,
        tags: form.tags,
        display_order: parseInt(form.display_order || "0", 10),
        featured: form.featured,
      };
      if (editing === "new") return contentClient.create<LinkCard>(slug, "links", payload);
      if (editing) return contentClient.update<LinkCard>(slug, "links", String(editing.id), payload);
      throw new Error("no row");
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["site", slug, "content", "links"] });
      setEditing(null);
      setForm(EMPTY);
      setError(null);
    },
    onError: (e) => setError(e instanceof Error ? e.message : "Save failed"),
  });

  const remove = useMutation({
    mutationFn: (id: number) => contentClient.delete(slug, "links", String(id)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "links"] }),
  });

  const openEdit = (l: LinkCard) => {
    setEditing(l);
    setForm({
      title: l.title,
      url: l.url,
      description_ko: l.description_ko ?? "",
      description_en: l.description_en ?? "",
      thumbnail_url: l.thumbnail_url ?? "",
      tags: l.tags ?? [],
      display_order: String(l.display_order),
      featured: l.featured,
    });
    setError(null);
  };

  const columns = [
    { key: "title", label: "Title" },
    {
      key: "url", label: "URL", width: "240px" as const,
      render: (row: unknown) => (
        <a href={str(field(row, "url"))} target="_blank" rel="noreferrer" className="text-xs text-muted truncate block hover:text-foreground">
          {str(field(row, "url"))}
        </a>
      ),
    },
    {
      key: "display_order", label: "Order", width: "60px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "display_order"))}</span>,
    },
    {
      key: "updated", label: "Updated", width: "160px" as const,
      render: (row: unknown) => <span className="text-muted text-xs">{str(field(row, "updated_at"))}</span>,
    },
    {
      key: "actions", label: "Actions", width: "100px" as const,
      render: (row: unknown) => {
        const r = row as LinkCard;
        return (
          <div className="flex justify-end gap-1">
            <button
              onClick={() => openEdit(r)}
              className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-foreground hover:bg-surface/50"
              aria-label="Edit"
            >
              <Pencil size={14} />
            </button>
            <button
              onClick={() => { if (confirm(`Delete "${r.title}"?`)) remove.mutate(r.id); }}
              className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-destructive-fg hover:bg-destructive-bg"
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
        <Input placeholder="Search links..." className="w-60" value={search} onChange={(e) => setSearch(e.target.value)} />
        <Button size="sm" onClick={() => { setEditing("new"); setForm(EMPTY); setError(null); }}>
          <Plus size={14} className="mr-1" /> New Link
        </Button>
      </div>
      <ContentTable
        columns={columns}
        data={filtered}
        isLoading={isLoading}
        emptyTitle="No links yet"
        emptyDescription="Add your first link."
      />

      <Drawer
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing === "new" ? "New Link" : "Edit Link"}
        description={editing !== null && editing !== "new" ? editing.title : "Add a link card to your lobby"}
        width="w-[520px]"
        footer={
          <>
            <Button variant="outline" onClick={() => setEditing(null)} disabled={save.isPending}>Cancel</Button>
            <Button onClick={() => save.mutate()} disabled={save.isPending || !form.title.trim() || !form.url.trim()}>
              {save.isPending ? "Saving..." : "Save"}
            </Button>
          </>
        }
      >
        <DrawerField label="Title" required>
          <Input value={form.title} onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))} autoFocus />
        </DrawerField>
        <DrawerField label="URL" required>
          <Input value={form.url} onChange={(e) => setForm((f) => ({ ...f, url: e.target.value }))} placeholder="https://..." type="url" />
        </DrawerField>
        <DrawerField label="Thumbnail">
          <ImageField
            slug={slug}
            extension="links"
            value={form.thumbnail_url}
            onChange={(v) => setForm((f) => ({ ...f, thumbnail_url: v ?? "" }))}
          />
        </DrawerField>
        <DrawerField label="Tags">
          <TagInput value={form.tags} onChange={(tags) => setForm((f) => ({ ...f, tags }))} />
        </DrawerField>
        <DrawerField label="Display order">
          <Input
            type="number"
            value={form.display_order}
            onChange={(e) => setForm((f) => ({ ...f, display_order: e.target.value }))}
          />
        </DrawerField>
        <DrawerField label="Description (Korean)">
          <Textarea value={form.description_ko} onChange={(e) => setForm((f) => ({ ...f, description_ko: e.target.value }))} rows={3} />
        </DrawerField>
        <DrawerField label="Description (English)">
          <Textarea value={form.description_en} onChange={(e) => setForm((f) => ({ ...f, description_en: e.target.value }))} rows={3} />
        </DrawerField>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={form.featured} onChange={(e) => setForm((f) => ({ ...f, featured: e.target.checked }))} />
          Featured
        </label>
        {error && <p className="text-sm text-destructive-fg">{error}</p>}
      </Drawer>
    </div>
  );
}
