import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { ApiError, contentClient, searchTmdb, type TmdbSearchResult } from "../shared/api";
import { ImageField } from "../shared/ui/ImageField";
import { ContentTable } from "../shared/content-table";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Textarea } from "../../shared/ui/textarea";
import { MarkdownEditor } from "../shared/ui/MarkdownEditor";
import { Drawer, DrawerField } from "../../shared/ui/drawer";
import { Pencil, Trash2, Send, Plus, X } from "lucide-react";
import { useRowFilter } from "../shared/useRowFilter";
import { field, str } from "../shared/row-utils";
import {
  listSeries, createSeries, showSeries, updateSeries, deleteSeries,
  type SeriesGroup, type SeriesGroupDetail,
} from "../shared/api";
import { Badge } from "../../shared/ui/badge";
import { Link } from "react-router";

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
  series_group_slug: string | null;
  series_order: number | null;
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

interface FormState {
  title: string;
  media_type: string;
  release_year: string;
  poster_path: string;
  watched_at: string;
  rating: string;
  review_ko: string;
  review_en: string;
  rewatch: boolean;
  series_group_slug: string;
  series_order: string;
}

const EMPTY: FormState = {
  title: "",
  media_type: "movie",
  release_year: "",
  poster_path: "",
  watched_at: "",
  rating: "7",
  review_ko: "",
  review_en: "",
  rewatch: false,
  series_group_slug: "",
  series_order: "",
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
  const [tab, setTab] = useState<"movies" | "series">("movies");

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
        series_group_slug: form.series_group_slug || null,
        series_order: form.series_order ? parseInt(form.series_order, 10) : null,
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
      poster_path: m.poster_path ?? "",
      watched_at: m.watched_at ?? "",
      rating: String(m.rating),
      review_ko: m.review_ko ?? "",
      review_en: m.review_en ?? "",
      rewatch: m.rewatch,
      series_group_slug: (m as any).series_group_slug ?? "",
      series_order: (m as any).series_order != null ? String((m as any).series_order) : "",
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
      <div className="flex gap-4 mb-4">
        <button
          onClick={() => setTab("movies")}
          className={`text-sm font-medium pb-1 border-b-2 ${tab === "movies" ? "border-[#22c55e] text-foreground" : "border-transparent text-muted"}`}
        >Movies</button>
        <button
          onClick={() => setTab("series")}
          className={`text-sm font-medium pb-1 border-b-2 ${tab === "series" ? "border-[#22c55e] text-foreground" : "border-transparent text-muted"}`}
        >Series</button>
      </div>

      {tab === "series" ? (
        <SeriesView slug={slug} />
      ) : (
        <>
      <div className="flex items-center justify-between mb-3">
        <div className="flex gap-2">
          <Input placeholder="Search movies..." className="w-60" value={search} onChange={(e) => setSearch(e.target.value)} />
          <TmdbSearchRow slug={slug} onPick={(r) => {
            setForm((f) => ({
              ...f,
              title: r.title,
              media_type: r.media_type,
              release_year: r.release_year != null ? String(r.release_year) : "",
              poster_path: r.poster_path ? `https://image.tmdb.org/t/p/w500${r.poster_path}` : "",
            }));
            setEditing("new");
            setError(null);
          }} />
        </div>
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
        <DrawerField label="Poster">
          <ImageField
            slug={slug}
            extension="movies"
            value={form.poster_path}
            onChange={(v) => setForm((f) => ({ ...f, poster_path: v ?? "" }))}
          />
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
          <MarkdownEditor slug={slug} extension="movies" value={form.review_ko} onChange={(v) => setForm((f) => ({ ...f, review_ko: v }))} rows={5} />
        </DrawerField>
        <DrawerField label="Review (English)">
          <MarkdownEditor slug={slug} extension="movies" value={form.review_en} onChange={(v) => setForm((f) => ({ ...f, review_en: v }))} rows={5} />
        </DrawerField>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={form.rewatch} onChange={(e) => setForm((f) => ({ ...f, rewatch: e.target.checked }))} />
          Rewatch
        </label>
        <SeriesField slug={slug} form={form} setForm={setForm} />
        {error && <p className="text-sm text-red-600">{error}</p>}
      </Drawer>
    </>
      )}
    </div>
  );
}

function SeriesField({ slug, form, setForm }: { slug: string; form: FormState; setForm: (cb: (prev: FormState) => FormState) => void }) {
  const { data: groups = [] } = useQuery({
    queryKey: ["site", slug, "movies", "series"],
    queryFn: () => listSeries(slug),
  });
  return (
    <>
      <DrawerField label="Series">
        <select
          value={form.series_group_slug}
          onChange={(e) => setForm((f) => ({ ...f, series_group_slug: e.target.value }))}
          className="h-10 w-full rounded-md border border-line bg-canvas px-3 text-sm text-foreground"
        >
          <option value="">None</option>
          {groups.map((g) => (
            <option key={g.slug} value={g.slug}>{g.title_ko ?? g.title_en ?? g.slug}</option>
          ))}
        </select>
      </DrawerField>
      <DrawerField label="Series Order">
        <Input
          type="number" min={0} value={form.series_order}
          onChange={(e) => setForm((f) => ({ ...f, series_order: e.target.value }))}
          placeholder="0"
        />
      </DrawerField>
    </>
  );
}

function SeriesView({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<null | SeriesGroup | "new">(null);
  const [seriesForm, setSeriesForm] = useState({ title_ko: "", title_en: "" });
  const [selected, setSelected] = useState<null | SeriesGroup>(null);

  const openEdit = (g: SeriesGroup) => {
    setEditing(g);
    setSeriesForm({ title_ko: g.title_ko ?? "", title_en: g.title_en ?? "" });
  };

  const { data: groups = [] } = useQuery({
    queryKey: ["site", slug, "movies", "series"],
    queryFn: () => listSeries(slug),
  });

  const { data: detail } = useQuery({
    queryKey: ["site", slug, "movies", "series", selected?.slug],
    queryFn: () => showSeries(slug, selected!.slug),
    enabled: !!selected,
  });

  const createMut = useMutation({
    mutationFn: () => createSeries(slug, seriesForm),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ["site", slug, "movies", "series"] }); setEditing(null); setSeriesForm({ title_ko: "", title_en: "" }); },
  });

  const updateMut = useMutation({
    mutationFn: () => updateSeries(slug, (editing as SeriesGroup).slug, seriesForm),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ["site", slug, "movies", "series"] }); setEditing(null); setSeriesForm({ title_ko: "", title_en: "" }); },
  });

  const handleSave = () => { if (editing === "new") createMut.mutate(); else updateMut.mutate(); };

  const deleteMut = useMutation({
    mutationFn: (s: string) => deleteSeries(slug, s),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ["site", slug, "movies", "series"] }); setSelected(null); },
  });

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <Input placeholder="Filter series..." className="w-60" />
        <Button size="sm" onClick={() => { setEditing("new"); setSeriesForm({ title_ko: "", title_en: "" }); }}>
          <Plus size={14} className="mr-1" /> New Series
        </Button>
      </div>

      <div className="grid grid-cols-1 gap-2 mb-4">
        {groups.map((g) => (
          <div
            key={g.slug}
            className={`border border-line rounded-lg p-3 flex items-center gap-3 cursor-pointer ${selected?.slug === g.slug ? "ring-1 ring-[#22c55e]" : ""}`}
            onClick={() => setSelected(selected?.slug === g.slug ? null : g)}
          >
            <div className="size-9 rounded-lg bg-[#fef3c7] text-[#92400e] flex items-center justify-center text-base font-bold shrink-0">
              {(g.title_ko ?? g.title_en ?? g.slug)[0].toUpperCase()}
            </div>
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium">{g.title_ko ?? g.title_en ?? g.slug}</div>
              <div className="text-xs text-muted">/{g.slug}</div>
            </div>
            {detail?.group.slug === g.slug && <Badge>{detail.entries.length} entries</Badge>}
            <button
              onClick={(e) => { e.stopPropagation(); openEdit(g); }}
              className="text-muted hover:text-foreground shrink-0"
            >
              <Pencil size={14} />
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); if (confirm(`Delete series "${g.title_ko ?? g.title_en ?? g.slug}"?`)) deleteMut.mutate(g.slug); }}
              className="text-red-500 hover:text-red-600 shrink-0"
            >
              <Trash2 size={14} />
            </button>
          </div>
        ))}
        {groups.length === 0 && (
          <div className="text-center py-8 text-muted text-sm">No series yet. Create your first one.</div>
        )}
      </div>

      {detail && (
        <div className="border border-line rounded-lg p-4">
          <h3 className="text-sm font-semibold mb-2">Members — {detail.group.title_ko ?? detail.group.title_en ?? detail.group.slug}</h3>
          {detail.entries.length === 0 ? (
            <p className="text-xs text-muted">No entries assigned to this series.</p>
          ) : (
            <div className="space-y-1">
              {detail.entries.map((e: any) => (
                <div key={e.id} className="flex items-center gap-2 text-sm px-2 py-1 rounded border border-line">
                  <span className="text-muted w-6 text-center shrink-0">{e.series_order ?? "—"}</span>
                  <span className="flex-1">{e.title}</span>
                  <Badge variant="outline" className="text-[10px]">{e.rating}/10</Badge>
                  <button
                    onClick={() => { if (confirm(`Remove "${e.title}" from this series?`)) contentClient.update(slug, "movies", e.slug, { series_group_slug: null, series_order: null }).then(() => qc.invalidateQueries({ queryKey: ["site", slug, "movies", "series"] })); }}
                    className="text-muted hover:text-red-500"
                    title="Remove from series"
                  >
                    <X size={12} />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      <Drawer
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing === "new" ? "New Series" : "Edit Series"}
        width="w-[480px]"
        footer={
          <>
            <Button variant="outline" onClick={() => setEditing(null)}>Cancel</Button>
            <Button onClick={() => handleSave()} disabled={createMut.isPending || updateMut.isPending || (!seriesForm.title_ko.trim() && !seriesForm.title_en.trim())}>
              {(createMut.isPending || updateMut.isPending) ? "Saving..." : editing === "new" ? "Create" : "Save"}
            </Button>
          </>
        }
      >
        <DrawerField label="Title (Korean)">
          <Input value={seriesForm.title_ko} onChange={(e) => setSeriesForm((f) => ({ ...f, title_ko: e.target.value }))} />
        </DrawerField>
        <DrawerField label="Title (English)">
          <Input value={seriesForm.title_en} onChange={(e) => setSeriesForm((f) => ({ ...f, title_en: e.target.value }))} />
        </DrawerField>
      </Drawer>
    </div>
  );
}

function TmdbSearchRow({ slug, onPick }: { slug: string; onPick: (r: TmdbSearchResult) => void }) {
  const [q, setQ] = useState("");
  const search = useQuery({
    queryKey: ["site", slug, "movies", "tmdb", q],
    queryFn: () => searchTmdb(slug, q),
    enabled: q.trim().length > 1,
  });
  return (
    <div className="relative">
      <Input
        placeholder="Search TMDB…"
        className="w-56"
        value={q}
        onChange={(e) => setQ(e.target.value)}
      />
      {search.isError && (search.error as ApiError)?.code === "tmdb_disabled" && (
        <p className="mt-1 text-xs text-muted max-w-56">
          TMDB 검색 비활성 —{" "}
          <Link className="underline" to={`/s/${slug}/settings`}>Settings</Link>
          에서 TMDB Key Env를 확인하거나 <code>OXIBUILDER_TMDB_KEY</code> 환경변수를
          설정하세요. 제목/포스터는 수동 입력할 수 있습니다.
        </p>
      )}
      {search.data && search.data.length > 0 && (
        <div className="absolute z-10 mt-1 w-64 max-h-64 overflow-auto rounded-md border border-line bg-surface shadow-lg">
          {search.data.map((r) => (
            <button
              key={r.tmdb_id}
              className="block w-full text-left px-3 py-2 text-sm hover:bg-surface/60"
              onClick={() => {
                onPick(r);
                setQ("");
              }}
            >
              <div className="font-medium">{r.title}</div>
              <div className="text-xs text-muted">
                {r.media_type} · {r.release_year ?? "—"}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
