// Markdown body editor with inline image insertion.
//
// Edit/Preview toggle (preview resolves `media/...` via the admin resolver).
// Image insertion: a toolbar button opens the MediaPicker; drag-and-drop or
// paste of an image file uploads it and splices `![alt](media/<ext>/<uuid>)`
// at the textarea selection. External http(s) image URLs typed inline render
// unchanged.

import { useMemo, useRef, useState } from "react";
import { Marked } from "marked";
import { ImagePlus } from "lucide-react";
import { Textarea } from "../../../shared/ui/textarea";
import { Button } from "../../../shared/ui/button";
import { adminAssetResolver } from "../../../shared/assets";
import { uploadImage } from "../api";
import { MediaPicker } from "./MediaPicker";

interface Props {
  value: string;
  onChange: (v: string) => void;
  /** Site slug — required for upload + resolver. */
  slug: string;
  /** Extension namespace for uploaded media (e.g. "blog"). */
  extension: string;
  rows?: number;
  placeholder?: string;
}

function isMediaRef(src: string): boolean {
  return /^\/?media\//.test(src.trim());
}

export function MarkdownEditor({
  value,
  onChange,
  slug,
  extension,
  rows = 6,
  placeholder,
}: Props) {
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const [pending, setPending] = useState(false);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const resolver = adminAssetResolver(slug);

  const html = useMemo(() => {
    const m = new Marked({ gfm: true, breaks: false });
    // Rewrite only media logical paths to the admin endpoint; leave external
    // URLs and anchors untouched. Mutates the token href in place so marked's
    // default image renderer (lazy-loading etc.) stays intact.
    m.use({
      walkTokens(token) {
        if (
          token.type === "image" &&
          typeof token.href === "string" &&
          isMediaRef(token.href)
        ) {
          const resolved = resolver.resolve(token.href);
          if (resolved) token.href = resolved;
        }
      },
    });
    return m.parse(value || "", { async: false }) as string;
  }, [value, resolver]);

  /** Splice `text` into the body at the current selection, then restore caret. */
  function splice(text: string) {
    const ta = taRef.current;
    if (!ta) {
      onChange(value + text);
      return;
    }
    const start = ta.selectionStart ?? value.length;
    const end = ta.selectionEnd ?? value.length;
    onChange(value.slice(0, start) + text + value.slice(end));
    const pos = start + text.length;
    requestAnimationFrame(() => {
      ta.focus();
      ta.setSelectionRange(pos, pos);
    });
  }

  async function uploadAndInsert(file: File) {
    setUploadError(null);
    if (!file.type.startsWith("image/")) {
      setUploadError("Only image files are supported.");
      return;
    }
    setPending(true);
    try {
      const media = await uploadImage(slug, extension, file);
      const alt = file.name.replace(/\.[^.]+$/, "");
      splice(`![${alt}](${media.path})\n`);
    } catch (err) {
      setUploadError(err instanceof Error ? err.message : "Upload failed");
    } finally {
      setPending(false);
    }
  }

  function onDrop(e: React.DragEvent) {
    const f = e.dataTransfer.files?.[0];
    if (f) {
      e.preventDefault();
      uploadAndInsert(f);
    }
  }

  function onPaste(e: React.ClipboardEvent) {
    const f = e.clipboardData.files?.[0];
    if (f) {
      e.preventDefault();
      uploadAndInsert(f);
    }
  }

  return (
    <div className="border border-line rounded overflow-hidden">
      <div className="flex items-center gap-0 border-b border-line bg-surface/30">
        <button
          type="button"
          onClick={() => setMode("edit")}
          className={`px-3 py-1 text-xs font-medium ${mode === "edit" ? "bg-canvas text-foreground" : "text-muted hover:text-foreground"}`}
        >
          Edit
        </button>
        <button
          type="button"
          onClick={() => setMode("preview")}
          className={`px-3 py-1 text-xs font-medium ${mode === "preview" ? "bg-canvas text-foreground" : "text-muted hover:text-foreground"}`}
        >
          Preview
        </button>
        <div className="ml-auto px-2">
          <MediaPicker
            slug={slug}
            extension={extension}
            onPick={(path) => splice(`![](${path})\n`)}
            trigger={
              <Button type="button" variant="ghost" size="sm" className="h-7 gap-1 px-2 text-xs">
                <ImagePlus className="size-3.5" />
                Image
              </Button>
            }
          />
        </div>
      </div>
      {mode === "edit" ? (
        <Textarea
          ref={taRef}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onDrop={onDrop}
          onPaste={onPaste}
          rows={rows}
          placeholder={placeholder}
          className="border-0 rounded-none"
        />
      ) : (
        <div
          className="p-3 text-sm prose prose-sm max-w-none"
          dangerouslySetInnerHTML={{ __html: html }}
        />
      )}
      {(pending || uploadError) && (
        <div className="px-3 py-1 text-xs border-t border-line">
          {pending && <span className="text-muted">Uploading…</span>}
          {uploadError && <span className="text-destructive-fg">{uploadError}</span>}
        </div>
      )}
    </div>
  );
}
