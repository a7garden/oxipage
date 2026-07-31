import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { contentClient } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Textarea } from "../../shared/ui/textarea";
import { ImageField } from "../shared/ui/ImageField";
import { MarkdownEditor } from "../shared/ui/MarkdownEditor";
import { Drawer, DrawerField } from "../../shared/ui/drawer";
import { Pencil, Trash2, Send, Plus } from "lucide-react";
import { useRowFilter } from "../shared/useRowFilter";
import { field, str } from "../shared/row-utils";

interface Book {
  id: number;
  source: string;
  external_id: string | null;
  isbn13: string | null;
  title: string;
  author: string | null;
  cover_image_url: string | null;
  rating: number;
  review_ko: string | null;
  review_en: string | null;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

interface FormState {
  title: string;
  author: string;
  isbn13: string;
  rating: string;
  review_ko: string;
  review_en: string;
  status: string;
  started_at: string;
  finished_at: string;
  cover_image_url: string;
}

const EMPTY: FormState = {
  title: "",
  author: "",
  isbn13: "",
  rating: "7",
  review_ko: "",
  review_en: "",
  status: "wishlist",
  started_at: "",
  finished_at: "",
  cover_image_url: "",
};

function StarRating({ rating }: { rating: unknown }) {
  const r = typeof rating === "number" ? Math.floor(rating / 2) : null;
  if (r == null) return <span className="text-xs text-muted">—</span>;
  return <span className="text-[#eab308] text-xs">{"★".repeat(r)}{"☆".repeat(5 - r)}</span>;
}

export function BooksTab({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<null | Book | "new">(null);
  const [form, setForm] = useState<FormState>(EMPTY);
  const [error, setError] = useState<string | null>(null);

  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "books"],
    queryFn: () => contentClient.list<Book>(slug, "books", { draft: true }),
  });

  const [search, setSearch] = useState("");
  const filtered = useRowFilter(data ?? [], search, (row) => [row.title, row.author ?? '']);

  const save = useMutation({
    mutationFn: async () => {
      const payload = {
        title: form.title.trim(),
        author: form.author || null,
        isbn13: form.isbn13 || null,
        rating: parseInt(form.rating || "0", 10),
        review_ko: form.review_ko || null,
        review_en: form.review_en || null,
        status: form.status,
        started_at: form.started_at || null,
        finished_at: form.finished_at || null,
      };
      if (editing === "new") return contentClient.create<Book>(slug, "books", payload);
      if (editing) return contentClient.update<Book>(slug, "books", String(editing.id), payload);
      throw new Error("no row");
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["site", slug, "content", "books"] });
      setEditing(null);
      setForm(EMPTY);
      setError(null);
    },
    onError: (e) => setError(e instanceof Error ? e.message : "Save failed"),
  });

  const publish = useMutation({
    mutationFn: (id: number) => contentClient.action<Book>(slug, "books", String(id), "publish"),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "books"] }),
  });

  const remove = useMutation({
    mutationFn: (id: number) => contentClient.delete(slug, "books", String(id)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "books"] }),
  });

  const openEdit = (b: Book) => {
    setEditing(b);
    setForm({
      title: b.title,
      author: b.author ?? "",
      isbn13: b.isbn13 ?? "",
      rating: String(b.rating),
      review_ko: b.review_ko ?? "",
      review_en: b.review_en ?? "",
      status: b.status,
      started_at: b.started_at ?? "",
      finished_at: b.finished_at ?? "",
      cover_image_url: b.cover_image_url ?? "",
    });
    setError(null);
  };

  const columns = [
    { key: "title", label: "Title" },
    {
      key: "author", label: "Author", width: "140px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "author"))}</span>,
    },
    {
      key: "rating", label: "Rating", width: "100px" as const,
      render: (row: unknown) => <StarRating rating={field(row, "rating")} />,
    },
    {
      key: "status", label: "Status", width: "100px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "status"))}</span>,
    },
    {
      key: "actions", label: "Actions", width: "140px" as const,
      render: (row: unknown) => {
        const r = row as Book;
        return (
          <div className="flex justify-end gap-1">
            <button
              onClick={() => publish.mutate(r.id)}
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
        <Input placeholder="Search books..." className="w-60" value={search} onChange={(e) => setSearch(e.target.value)} />
        <Button size="sm" onClick={() => { setEditing("new"); setForm(EMPTY); setError(null); }}>
          <Plus size={14} className="mr-1" /> Add Review
        </Button>
      </div>
      <ContentTable
        columns={columns}
        data={filtered}
        isLoading={isLoading}
        emptyTitle="No book reviews yet"
        emptyDescription="Add your first book review."
      />

      <Drawer
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing === "new" ? "Add Book Review" : "Edit Book Review"}
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
        <div className="grid grid-cols-2 gap-3">
          <DrawerField label="Author">
            <Input value={form.author} onChange={(e) => setForm((f) => ({ ...f, author: e.target.value }))} />
          </DrawerField>
          <DrawerField label="ISBN13">
            <Input value={form.isbn13} onChange={(e) => setForm((f) => ({ ...f, isbn13: e.target.value }))} placeholder="978..." />
          </DrawerField>
        </div>
        <div className="grid grid-cols-3 gap-3">
          <DrawerField label="Rating" hint="0–10">
            <Input
              type="number"
              min={0}
              max={10}
              value={form.rating}
              onChange={(e) => setForm((f) => ({ ...f, rating: e.target.value }))}
            />
          </DrawerField>
          <DrawerField label="Status">
            <select
              value={form.status}
              onChange={(e) => setForm((f) => ({ ...f, status: e.target.value }))}
              className="h-10 w-full rounded-md border border-line bg-canvas px-3 text-sm text-foreground"
            >
              <option value="wishlist">wishlist</option>
              <option value="reading">reading</option>
              <option value="completed">completed</option>
              <option value="dropped">dropped</option>
            </select>
          </DrawerField>
        </div>
        <DrawerField label="Cover">
          <ImageField
            slug={slug}
            extension="books"
            value={form.cover_image_url}
            onChange={(v) => setForm((f) => ({ ...f, cover_image_url: v ?? "" }))}
          />
        </DrawerField>
        <div className="grid grid-cols-2 gap-3">
          <DrawerField label="Started" hint="YYYY-MM-DD">
            <Input type="date" value={form.started_at} onChange={(e) => setForm((f) => ({ ...f, started_at: e.target.value }))} />
          </DrawerField>
          <DrawerField label="Finished" hint="YYYY-MM-DD">
            <Input type="date" value={form.finished_at} onChange={(e) => setForm((f) => ({ ...f, finished_at: e.target.value }))} />
          </DrawerField>
        </div>
        <DrawerField label="Review (Korean)">
          <MarkdownEditor value={form.review_ko} onChange={(v) => setForm((f) => ({ ...f, review_ko: v }))} rows={5} />
        </DrawerField>
        <DrawerField label="Review (English)">
          <MarkdownEditor value={form.review_en} onChange={(v) => setForm((f) => ({ ...f, review_en: v }))} rows={5} />
        </DrawerField>
        {error && <p className="text-sm text-red-600">{error}</p>}
      </Drawer>
    </div>
  );
}
