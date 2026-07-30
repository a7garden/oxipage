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

interface BlogPost {
  id: number;
  slug: string;
  title: string;
  body: string;
  lang: string;
  translation_group_id: number | null;
  tags: string[];
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

const EMPTY: { title: string; body: string; lang: string; tags: string } = {
  title: "",
  body: "",
  lang: "ko",
  tags: "",
};

export function BlogTab({ slug }: { slug: string }) {
  const qc = useQueryClient();
  const [editing, setEditing] = useState<null | BlogPost | "new">(null);
  const [form, setForm] = useState(EMPTY);
  const [error, setError] = useState<string | null>(null);

  const { data, isLoading } = useQuery({
    queryKey: ["site", slug, "content", "blog"],
    queryFn: () => contentClient.list<BlogPost>(slug, "blog", { draft: true }),
  });

  const [search, setSearch] = useState("");
  const filtered = useRowFilter(data ?? [], search, (row) => [row.title, row.slug]);

  const save = useMutation({
    mutationFn: async () => {
      const payload = {
        title: form.title.trim(),
        body: form.body,
        lang: form.lang,
        tags: form.tags.split(",").map((t) => t.trim()).filter(Boolean),
      };
      if (editing === "new") {
        return contentClient.create<BlogPost>(slug, "blog", payload);
      }
      if (editing) {
        return contentClient.update<BlogPost>(slug, "blog", editing.slug, payload);
      }
      throw new Error("no row");
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["site", slug, "content", "blog"] });
      setEditing(null);
      setForm(EMPTY);
      setError(null);
    },
    onError: (e) => setError(e instanceof Error ? e.message : "Save failed"),
  });

  const publish = useMutation({
    mutationFn: (postSlug: string) => contentClient.action<BlogPost>(slug, "blog", postSlug, "publish"),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "blog"] }),
  });

  const remove = useMutation({
    mutationFn: (postSlug: string) => contentClient.delete(slug, "blog", postSlug),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["site", slug, "content", "blog"] }),
  });

  const openNew = () => {
    setEditing("new");
    setForm(EMPTY);
    setError(null);
  };

  const openEdit = (post: BlogPost) => {
    setEditing(post);
    setForm({
      title: post.title,
      body: post.body ?? "",
      lang: post.lang,
      tags: (post.tags ?? []).join(", "),
    });
    setError(null);
  };

  const columns = [
    {
      key: "title", label: "Title",
      render: (row: unknown) => (
        <div>
          <div className="font-medium">{str(field(row, "title"))}</div>
          <div className="text-xs text-muted">/{str(field(row, "slug"))}</div>
        </div>
      ),
    },
    {
      key: "status", label: "Status", width: "96px" as const,
      render: (row: unknown) => (
        <Badge variant={field(row, "published_at") ? "positive" : "secondary"}>
          {field(row, "published_at") ? "Published" : "Draft"}
        </Badge>
      ),
    },
    {
      key: "lang", label: "Lang", width: "60px" as const,
      render: (row: unknown) => <span className="text-muted text-xs">{str(field(row, "lang"))}</span>,
    },
    {
      key: "tags", label: "Tags", width: "180px" as const,
      render: (row: unknown) => {
        const tags = field(row, "tags");
        return (
          <span className="text-xs text-muted">
            {Array.isArray(tags) ? tags.join(", ") : "—"}
          </span>
        );
      },
    },
    {
      key: "updated", label: "Updated", width: "160px" as const,
      render: (row: unknown) => <span className="text-muted text-xs">{str(field(row, "updated_at") || field(row, "published_at"))}</span>,
    },
    {
      key: "actions", label: "Actions", width: "140px" as const,
      render: (row: unknown) => {
        const r = row as BlogPost;
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
                if (confirm(`Delete "${r.title}"?`)) remove.mutate(r.slug);
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
        <Input placeholder="Search posts..." className="w-60" value={search} onChange={(e) => setSearch(e.target.value)} />
        <Button size="sm" onClick={openNew}>
          <Plus size={14} className="mr-1" /> New Post
        </Button>
      </div>
      <ContentTable
        columns={columns}
        data={filtered}
        isLoading={isLoading}
        emptyTitle="No posts yet"
        emptyDescription="Write your first blog post."
      />

      <Drawer
        open={editing !== null}
        onClose={() => setEditing(null)}
        title={editing === "new" ? "New Post" : "Edit Post"}
        description={editing !== null && editing !== "new" ? `/${editing.slug}` : "Create a new blog post draft"}
        width="w-[560px]"
        footer={
          <>
            <Button variant="outline" onClick={() => setEditing(null)} disabled={save.isPending}>
              Cancel
            </Button>
            <Button onClick={() => save.mutate()} disabled={save.isPending || !form.title.trim()}>
              {save.isPending ? "Saving..." : "Save"}
            </Button>
          </>
        }
      >
        <DrawerField label="Title" required>
          <Input
            value={form.title}
            onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))}
            placeholder="Post title"
            autoFocus
          />
        </DrawerField>
        <DrawerField label="Language">
          <select
            value={form.lang}
            onChange={(e) => setForm((f) => ({ ...f, lang: e.target.value }))}
            className="h-10 w-full rounded-md border border-line bg-canvas px-3 text-sm text-foreground"
          >
            <option value="ko">ko</option>
            <option value="en">en</option>
          </select>
        </DrawerField>
        <DrawerField label="Tags" hint="Comma-separated, e.g. rust, web, weekly">
          <Input
            value={form.tags}
            onChange={(e) => setForm((f) => ({ ...f, tags: e.target.value }))}
            placeholder="rust, web, ..."
          />
        </DrawerField>
        <DrawerField label="Body" hint="Markdown is supported">
          <Textarea
            value={form.body}
            onChange={(e) => setForm((f) => ({ ...f, body: e.target.value }))}
            rows={16}
            className="font-mono"
          />
        </DrawerField>
        {error && <p className="text-sm text-red-600">{error}</p>}
      </Drawer>
    </div>
  );
}
