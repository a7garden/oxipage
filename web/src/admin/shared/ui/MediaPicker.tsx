// Reusable media library picker — browse, upload, delete, and pick an
// uploaded asset. Renders its `trigger` via Radix Slot (so a <Button> trigger
// stays a real button) and opens a centered modal that mirrors the Drawer's
// overlay/escape language. Used by MarkdownEditor's image toolbar.

import { useEffect, useState, type ReactNode } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Slot } from "@radix-ui/react-slot";
import { Trash2, Upload, X } from "lucide-react";
import { adminAssetResolver } from "../../../shared/assets";
import {
  deleteMedia,
  listMedia,
  uploadImage,
  type MediaItem,
} from "../api";
import { Button } from "../../../shared/ui/button";

const EXTENSIONS = [
  "blog",
  "projects",
  "profile",
  "novels",
  "books",
  "movies",
  "scraps",
];

interface MediaPickerProps {
  slug: string;
  /** Namespace to filter by initially; undefined shows all. */
  extension?: string;
  /** Called with the chosen logical `media/...` path. */
  onPick: (path: string) => void;
  /** A button element; Slot merges the open handler into it. */
  trigger: ReactNode;
}

export function MediaPicker({ slug, extension, onPick, trigger }: MediaPickerProps) {
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [ext, setExt] = useState<string | undefined>(extension);
  const resolver = adminAssetResolver(slug);

  const { data, isLoading } = useQuery({
    queryKey: ["media", slug, ext],
    queryFn: () => listMedia(slug, ext),
    enabled: open,
  });

  const del = useMutation({
    mutationFn: (item: MediaItem) => deleteMedia(slug, item.extension, item.file),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["media", slug] }),
  });

  const upload = useMutation({
    mutationFn: (file: File) => uploadImage(slug, ext ?? "blog", file),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["media", slug] }),
  });

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open]);

  function onUpload(e: React.ChangeEvent<HTMLInputElement>) {
    const f = e.target.files?.[0];
    if (f) upload.mutate(f);
    e.target.value = "";
  }

  function pick(path: string) {
    onPick(path);
    setOpen(false);
  }

  const triggerEl = (
    <Slot
      onClick={(e: React.MouseEvent) => {
        e.preventDefault();
        setOpen(true);
      }}
    >
      {trigger}
    </Slot>
  );

  if (!open) return triggerEl;

  return (
    <>
      {triggerEl}
      <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div className="absolute inset-0 bg-black/40" onClick={() => setOpen(false)} aria-hidden />
        <div
          className="relative w-[680px] max-w-full max-h-[82vh] flex flex-col rounded-lg border border-line bg-canvas shadow-2xl"
          role="dialog"
          aria-modal="true"
          aria-label="Media library"
        >
          <div className="flex items-center justify-between px-4 py-3 border-b border-line">
            <h2 className="text-base font-semibold text-foreground">Media library</h2>
            <button
              onClick={() => setOpen(false)}
              className="inline-flex items-center justify-center size-7 rounded-md text-muted hover:text-foreground hover:bg-surface/50"
              aria-label="Close"
            >
              <X size={16} />
            </button>
          </div>

          <div className="flex items-center gap-3 px-4 py-3 border-b border-line">
            <select
              className="border border-line rounded px-2 py-1.5 text-sm bg-surface text-foreground"
              value={ext ?? ""}
              onChange={(e) => setExt(e.target.value || undefined)}
            >
              <option value="">All namespaces</option>
              {EXTENSIONS.map((e) => (
                <option key={e} value={e}>
                  {e}
                </option>
              ))}
            </select>
            <label className="inline-flex items-center gap-1.5 text-sm cursor-pointer text-primary hover:underline">
              <Upload className="size-4" />
              <span>Upload</span>
              <input
                type="file"
                accept="image/png,image/jpeg,image/webp,image/gif"
                className="hidden"
                onChange={onUpload}
              />
            </label>
            {upload.isPending && <span className="text-xs text-muted">Uploading…</span>}
            {upload.isError && <span className="text-xs text-destructive-fg">Upload failed</span>}
          </div>

          <div className="flex-1 overflow-y-auto p-4">
            {isLoading ? (
              <p className="text-sm text-muted">Loading…</p>
            ) : !data || data.length === 0 ? (
              <p className="text-sm text-muted py-8 text-center">
                No media yet. Upload an image to insert.
              </p>
            ) : (
              <div className="grid grid-cols-3 sm:grid-cols-4 gap-3">
                {data.map((item) => (
                  <div
                    key={item.path}
                    className="group relative border border-line rounded overflow-hidden bg-surface"
                  >
                    <button
                      type="button"
                      className="block w-full aspect-square"
                      title={`Insert ${item.file}`}
                      onClick={() => pick(item.path)}
                    >
                      <img
                        src={resolver.resolve(item.path) ?? ""}
                        alt={item.file}
                        className="w-full h-full object-cover"
                        loading="lazy"
                      />
                    </button>
                    <button
                      type="button"
                      className="absolute top-1 right-1 rounded bg-black/50 p-1 text-white opacity-0 group-hover:opacity-100 transition-opacity"
                      title="Delete"
                      onClick={() => {
                        if (
                          confirm(
                            "Delete this asset? It may be referenced in content — broken <img> tags will 404.",
                          )
                        ) {
                          del.mutate(item);
                        }
                      }}
                    >
                      <Trash2 className="size-3.5" />
                    </button>
                    <span className="block px-1.5 py-1 text-[10px] text-muted truncate" title={item.file}>
                      {item.file}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="flex justify-end px-4 py-3 border-t border-line bg-surface/30">
            <Button variant="secondary" size="sm" onClick={() => setOpen(false)}>
              Close
            </Button>
          </div>
        </div>
      </div>
    </>
  );
}
