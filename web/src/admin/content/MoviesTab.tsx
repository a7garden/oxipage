import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { contentClient } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Textarea } from "../../shared/ui/textarea";
import { Drawer, DrawerField } from "../../shared/ui/drawer";
import { Pencil, Trash2, Send, Plus } from "lucide-react";
import { useRowFilter } from "../shared/useRowFilter";
import { field, str } from "../shared/row-utils";

interface MovieEntry {
  id: number;
  slug: string;
  tmdb_id: number | null;
  media_type: string;
  title: string;
  poster_path: string | null;
  release_year: number | null;
  watched_at: string | null;
  rating: number;
  review_ko: string | null;
  review_en: string | null;
  rewatch: boolean;
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

interface FormState {
  title: string;
  media_type: string;
  release_year: string;
  watched_at: string;
  rating: string;
  review_ko: string;
  review_en: string;
  rewatch: boolean;
}

const EMPTY: FormState = {
  title: "",
  media_type: "movie",
  release_year: "",
  watched_at: "",
  rating: "7",
  review_ko: "",
  review_en: "",
  rewatch: false,
};

function StarRating({ rating }: { rating: unknown }) {
  const r = typeof rating === "number" ? Math.floor(rating / 2) : null;
  if (r == null) return <span className="text-xs text-muted">—</span>;
  return <span className="text-[#eab308] text-xs">{"★".repeat(r)}{"☆".repeat(5 - r)}</span>;
}

export function MoviesTab({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<null | MovieEntry | "new">(null);
  const [form, setForm] = useState<FormState>(EMPTY);
  const [error, setError] = useState<string | null>(null);

  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "movies"],
    queryFn: () => contentClient.list<MovieEntry>(slug, "movies", { draft: true }),
  });

  const [search, setSearch] = useState("");
  const filtered = useRowFilter(data ?? [], search, (row) => [row.title, row.slug]);

  const save = useMutation({
    mutationFn: async () => {
      const payload = {
        title: form.title.trim(),
        media_type: form.media_type,
        release_year: form.release_year ? parseInt(form.release_year, 10) : null,
        watched_at: form.watched_at || null,
        rating: parseInt(form.rating || "0", 10),
        review_ko: form.review_ko || null,
        review_en: form.review_en || null,
        rewatch: form.rewatch,
      };
      if (editing === "new") return contentClient.create<MovieEntry>(slug, "movies", payload);
      if (editing) return contentClient.update<MovieEntry>(slug, "movies", editing.slug, payload);
      throw new Error("no row");
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["site", slug, "content", "movies"] });
      setEditing(null);
      setForm(EMPTY);
      setError(null);
    },
    onError: (e) => setError(e instanceof Error ? e.message : "Save failed"),
  });

  const publish = useMutation({
    mutationFn: (s: string) => contentClient.action<MovieEntry>(slug, "movies", s, "publish"),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "movies"] }),
  });

  const remove = useMutation({
    mutationFn: (s: string) => contentClient.delete(slug, "movies", s),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "movies"] }),
  });

  const openEdit = (m: MovieEntry) => {
    setEditing(m);
    setForm({
      title: m.title,
      media_type: m.media_type,
      release_year: m.release_year ? String(m.release_year) : "",
      watched_at: m.watched_at ?? "",
      rating: String(m.rating),
      review_ko: m.review_ko ?? "",
      review_en: m.review_en ?? "",
      rewatch: m.rewatch,
    });
    setError(null);
  };

  const columns = [
    {
      key: "title", label: "Title",
      render: (row: unknown) => {
        const title = str(field(row, "title"));
        const year = field(row, "release_year");
        return (
          <span>
            {title}
            {year ? <span className="text-muted text-xs ml-1">({str(year)})</span> : null}
          </span>
        );
      },
    },
    {
      key: "rating", label: "Rating", width: "100px" as const,
      render: (row: unknown) => <StarRating rating={field(row, "rating")} />,
    },
    {
      key: "watched", label: "Watched", width: "120px" as const,
      render: (row: unknown) => <span className="text-xs text-muted">{str(field(row, "watched_at") || field(row, "published_at"))}</span>,
    },
    {
      key: "actions", label: "Actions", width: "140px" as const,
      render: (row: unknown) => {
        const r = row as MovieEntry;
        return (
          <div className="flex justify-end gap-1">
            <button
              onClick={() => publish.mutate(r.slug)}
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
        <Input placeholder="Search movies..." className="w-60" value={search} onChange={(e) => setSearch(e.target.value)} />
        <Button size="sm" onClick={() => { setEditing("new"); setForm(EMPTY); setError(null); }}>
          <Plus size={14} className="mr-1" /> Add Review
        </Button>
      </div>
      <ContentTable
        columns={columns}
        data={filtered}
        isLoading={isLoading}
        emptyTitle="No movie reviews yet"
        emptyDescription="Add your first movie review."
      />

      <Drawer
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing === "new" ? "Add Movie Review" : "Edit Movie Review"}
        description={editing !== null && editing !== "new" ? `/${editing.slug}` : "Track a movie or TV show"}
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
          <DrawerField label="Type">
            <select
              value={form.media_type}
              onChange={(e) => setForm((f) => ({ ...f, media_type: e.target.value }))}
              className="h-10 w-full rounded-md border border-line bg-canvas px-3 text-sm text-foreground"
            >
              <option value="movie">movie</option>
              <option value="tv">tv</option>
            </select>
          </DrawerField>
          <DrawerField label="Release year">
            <Input
              type="number"
              value={form.release_year}
              onChange={(e) => setForm((f) => ({ ...f, release_year: e.target.value }))}
              placeholder="2024"
            />
          </DrawerField>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <DrawerField label="Watched on" hint="YYYY-MM-DD">
            <Input
              type="date"
              value={form.watched_at}
              onChange={(e) => setForm((f) => ({ ...f, watched_at: e.target.value }))}
            />
          </DrawerField>
          <DrawerField label="Rating" hint="0–10">
            <Input
              type="number"
              min={0}
              max={10}
              value={form.rating}
              onChange={(e) => setForm((f) => ({ ...f, rating: e.target.value }))}
            />
          </DrawerField>
        </div>
        <DrawerField label="Review (Korean)">
          <Textarea value={form.review_ko} onChange={(e) => setForm((f) => ({ ...f, review_ko: e.target.value }))} rows={5} />
        </DrawerField>
        <DrawerField label="Review (English)">
          <Textarea value={form.review_en} onChange={(e) => setForm((f) => ({ ...f, review_en: e.target.value }))} rows={5} />
        </DrawerField>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={form.rewatch} onChange={(e) => setForm((f) => ({ ...f, rewatch: e.target.checked }))} />
          Rewatch
        </label>
        {error && <p className="text-sm text-red-600">{error}</p>}
      </Drawer>
    </div>
  );
}
