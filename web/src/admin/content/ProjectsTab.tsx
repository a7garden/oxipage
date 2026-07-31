import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { contentClient } from "../shared/api";
import { ContentTable } from "../shared/content-table";
import { Badge } from "../../shared/ui/badge";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Textarea } from "../../shared/ui/textarea";
import { ImageField } from "../shared/ui/ImageField";
import { Drawer, DrawerField } from "../../shared/ui/drawer";
import { Pencil, Trash2, Send, Plus, ChevronDown, ChevronUp } from "lucide-react";
import { useRowFilter } from "../shared/useRowFilter";
import { field, str } from "../shared/row-utils";
import { adminAssetResolver } from "../../shared/assets";
import {
  addScreenshot, updateScreenshot, deleteScreenshot, showExtension,
  type Screenshot,
} from "../shared/api";

interface Project {
  id: number;
  slug: string;
  title_ko: string | null;
  title_en: string | null;
  description_ko: string | null;
  description_en: string | null;
  tech_stack: string[];
  status: string;
  started_at: string | null;
  ended_at: string | null;
  featured: boolean;
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

interface FormState {
  title_ko: string;
  title_en: string;
  description_ko: string;
  description_en: string;
  tech_stack: string;
  status: string;
  started_at: string;
  ended_at: string;
  featured: boolean;
}

const EMPTY: FormState = {
  title_ko: "",
  title_en: "",
  description_ko: "",
  description_en: "",
  tech_stack: "",
  status: "wip",
  started_at: "",
  ended_at: "",
  featured: false,
};

export function ProjectsTab({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<null | Project | "new">(null);
  const [form, setForm] = useState<FormState>(EMPTY);
  const [error, setError] = useState<string | null>(null);

  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "projects"],
    queryFn: () => contentClient.list<Project>(slug, "projects", { draft: true }),
  });

  const [search, setSearch] = useState("");
  const filtered = useRowFilter(data ?? [], search, (row) => [row.title_ko ?? '', row.title_en ?? '', row.slug]);

  const save = useMutation({
    mutationFn: async () => {
      const payload = {
        title_ko: form.title_ko.trim() || null,
        title_en: form.title_en.trim() || null,
        description_ko: form.description_ko || null,
        description_en: form.description_en || null,
        tech_stack: form.tech_stack.split(",").map((t) => t.trim()).filter(Boolean),
        status: form.status,
        started_at: form.started_at || null,
        ended_at: form.ended_at || null,
        featured: form.featured,
      };
      if (editing === "new") return contentClient.create<Project>(slug, "projects", payload);
      if (editing) return contentClient.update<Project>(slug, "projects", editing.slug, payload);
      throw new Error("no row");
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["site", slug, "content", "projects"] });
      setEditing(null);
      setForm(EMPTY);
      setError(null);
    },
    onError: (e) => setError(e instanceof Error ? e.message : "Save failed"),
  });

  const publish = useMutation({
    mutationFn: (s: string) => contentClient.action<Project>(slug, "projects", s, "publish"),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "projects"] }),
  });

  const remove = useMutation({
    mutationFn: (s: string) => contentClient.delete(slug, "projects", s),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "projects"] }),
  });

  const [showScreenshots, setShowScreenshots] = useState(false);
  const projectSlug = editing && editing !== "new" ? editing.slug : undefined;
  const { data: projectDetail, refetch: refetchDetail } = useQuery({
    queryKey: ["site", slug, "projects", projectSlug],
    queryFn: () => showExtension<{ project: Project; screenshots: Screenshot[] }>(slug!, "projects", projectSlug!),
    enabled: !!projectSlug && showScreenshots,
  });
  const screenshots = projectDetail?.screenshots ?? [];

  const addShotMut = useMutation({
    mutationFn: (input: { url: string; alt_ko?: string; alt_en?: string }) =>
      addScreenshot(slug!, projectSlug!, input),
    onSuccess: () => { refetchDetail(); setAddingScreenshot(false); setAddShotForm({ url: "", alt_ko: "", alt_en: "" }); },
  });

  const updateShotMut = useMutation({
    mutationFn: ({ sid, patch }: { sid: number; patch: { alt_ko?: string; alt_en?: string; display_order?: number } }) =>
      updateScreenshot(slug!, projectSlug!, sid, patch),
    onSuccess: () => refetchDetail(),
  });

  const deleteShotMut = useMutation({
    mutationFn: (sid: number) => deleteScreenshot(slug!, projectSlug!, sid),
    onSuccess: () => refetchDetail(),
  });

  const [addingScreenshot, setAddingScreenshot] = useState(false);
  const [addShotForm, setAddShotForm] = useState({ url: "", alt_ko: "", alt_en: "" });

  const openEdit = (p: Project) => {
    setEditing(p);
    setForm({
      title_ko: p.title_ko ?? "",
      title_en: p.title_en ?? "",
      description_ko: p.description_ko ?? "",
      description_en: p.description_en ?? "",
      tech_stack: (p.tech_stack ?? []).join(", "),
      status: p.status,
      started_at: p.started_at ?? "",
      ended_at: p.ended_at ?? "",
      featured: p.featured,
    });
    setError(null);
  };

  const columns = [
    {
      key: "title", label: "Title",
      render: (row: unknown) => {
        const r = row as Project;
        return (
          <div>
            <div className="font-medium">{r.title_ko || r.title_en || "—"}</div>
            <div className="text-xs text-muted">/{r.slug}</div>
          </div>
        );
      },
    },
    {
      key: "status", label: "Status", width: "80px" as const,
      render: (row: unknown) => (
        <Badge variant={str(field(row, "status")) === "active" ? "positive" : "secondary"}>
          {str(field(row, "status"))}
        </Badge>
      ),
    },
    {
      key: "tech_stack", label: "Tech", width: "160px" as const,
      render: (row: unknown) => {
        const tech = field(row, "tech_stack");
        return (
          <span className="text-xs text-muted">
            {Array.isArray(tech) ? tech.join(", ") : "—"}
          </span>
        );
      },
    },
    {
      key: "updated", label: "Updated", width: "160px" as const,
      render: (row: unknown) => <span className="text-muted text-xs">{str(field(row, "updated_at"))}</span>,
    },
    {
      key: "actions", label: "Actions", width: "140px" as const,
      render: (row: unknown) => {
        const r = row as Project;
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
              onClick={() => {
                if (confirm(`Delete "${r.title_ko || r.title_en}"?`)) remove.mutate(r.slug);
              }}
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
        <Input placeholder="Search projects..." className="w-60" value={search} onChange={(e) => setSearch(e.target.value)} />
        <Button size="sm" onClick={() => { setEditing("new"); setForm(EMPTY); setError(null); }}>
          <Plus size={14} className="mr-1" /> New Project
        </Button>
      </div>
      <ContentTable
        columns={columns}
        data={filtered}
        isLoading={isLoading}
        emptyTitle="No projects yet"
        emptyDescription="Add your first project."
      />

      <Drawer
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing === "new" ? "New Project" : "Edit Project"}
        description={editing !== null && editing !== "new" ? `/${editing.slug}` : "Create a new project entry"}
        width="w-[560px]"
        footer={
          <>
            <Button variant="outline" onClick={() => setEditing(null)} disabled={save.isPending}>
              Cancel
            </Button>
            <Button
              onClick={() => save.mutate()}
              disabled={save.isPending || (!form.title_ko.trim() && !form.title_en.trim())}
            >
              {save.isPending ? "Saving..." : "Save"}
            </Button>
          </>
        }
      >
        <DrawerField label="Title (Korean)">
          <Input value={form.title_ko} onChange={(e) => setForm((f) => ({ ...f, title_ko: e.target.value }))} placeholder="프로젝트 이름" />
        </DrawerField>
        <DrawerField label="Title (English)">
          <Input value={form.title_en} onChange={(e) => setForm((f) => ({ ...f, title_en: e.target.value }))} placeholder="Project name" />
        </DrawerField>
        <DrawerField label="Tech stack" hint="Comma-separated">
          <Input value={form.tech_stack} onChange={(e) => setForm((f) => ({ ...f, tech_stack: e.target.value }))} placeholder="rust, typescript, axum" />
        </DrawerField>
        <DrawerField label="Status">
          <select
            value={form.status}
            onChange={(e) => setForm((f) => ({ ...f, status: e.target.value }))}
            className="h-10 w-full rounded-md border border-line bg-canvas px-3 text-sm text-foreground"
          >
            <option value="wip">wip</option>
            <option value="active">active</option>
            <option value="archived">archived</option>
          </select>
        </DrawerField>
        <div className="grid grid-cols-2 gap-3">
          <DrawerField label="Started" hint="YYYY-MM-DD">
            <Input type="date" value={form.started_at} onChange={(e) => setForm((f) => ({ ...f, started_at: e.target.value }))} />
          </DrawerField>
          <DrawerField label="Ended" hint="YYYY-MM-DD">
            <Input type="date" value={form.ended_at} onChange={(e) => setForm((f) => ({ ...f, ended_at: e.target.value }))} />
          </DrawerField>
        </div>
        <DrawerField label="Description (Korean)">
          <Textarea value={form.description_ko} onChange={(e) => setForm((f) => ({ ...f, description_ko: e.target.value }))} rows={4} />
        </DrawerField>
        <DrawerField label="Description (English)">
          <Textarea value={form.description_en} onChange={(e) => setForm((f) => ({ ...f, description_en: e.target.value }))} rows={4} />
        </DrawerField>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={form.featured}
            onChange={(e) => setForm((f) => ({ ...f, featured: e.target.checked }))}
          />
          Featured
        </label>

        {projectSlug && (
          <div className="border-t border-line pt-4 mt-4">
            <button
              onClick={() => setShowScreenshots(!showScreenshots)}
              className="flex items-center gap-2 text-sm font-semibold text-foreground mb-3 w-full text-left"
            >
              <ChevronDown size={14} className={`transition-transform ${showScreenshots ? "" : "-rotate-90"}`} />
              Screenshots ({screenshots.length})
            </button>
            {showScreenshots && (
              <div className="space-y-2 mb-3">
                {screenshots.map((s: Screenshot, i: number) => (
                  <div key={s.id} className="flex items-center gap-2 p-2 border border-line rounded">
                    <img src={adminAssetResolver(slug).resolve(s.url) ?? ""} alt="" className="size-10 rounded object-cover shrink-0 border border-line" />
                    <input
                      className="flex-1 text-xs border border-line rounded px-1 py-0.5 bg-canvas"
                      defaultValue={s.alt_ko ?? ""}
                      onBlur={(e) => { if (e.target.value !== (s.alt_ko ?? "")) updateShotMut.mutate({ sid: s.id, patch: { alt_ko: e.target.value } }); }}
                      placeholder="alt (KO)"
                    />
                    <input
                      type="number"
                      className="w-12 text-xs border border-line rounded px-1 py-0.5 bg-canvas text-center"
                      defaultValue={s.display_order}
                      onBlur={(e) => { const v = Number(e.target.value); if (v !== s.display_order) updateShotMut.mutate({ sid: s.id, patch: { display_order: v } }); }}
                    />
                    <span className="text-xs text-muted">#{s.display_order}</span>
                    <button onClick={() => deleteShotMut.mutate(s.id)} className="text-red-500 hover:text-red-600">
                      <Trash2 size={14} />
                    </button>
                  </div>
                ))}
              </div>
            )}
            <button
              onClick={() => setAddingScreenshot(!addingScreenshot)}
              className="flex items-center gap-1 text-xs text-muted hover:text-foreground"
            >
              <Plus size={12} /> {addingScreenshot ? "Cancel" : "Add Screenshot"}
            </button>
            {addingScreenshot && (
              <div className="border border-line rounded p-3 mt-2 space-y-2 bg-surface/30">
                <ImageField
                  slug={slug}
                  extension="projects"
                  value={addShotForm.url}
                  onChange={(v) => setAddShotForm((f) => ({ ...f, url: v ?? "" }))}
                />
                <Input
                  placeholder="Alt text (Korean)"
                  value={addShotForm.alt_ko}
                  onChange={(e) => setAddShotForm((f) => ({ ...f, alt_ko: e.target.value }))}
                />
                <div className="flex gap-2 justify-end">
                  <Button size="sm" onClick={() => addShotMut.mutate({ url: addShotForm.url, alt_ko: addShotForm.alt_ko || undefined, alt_en: addShotForm.alt_en || undefined })} disabled={!addShotForm.url.trim() || addShotMut.isPending}>
                    Add
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
