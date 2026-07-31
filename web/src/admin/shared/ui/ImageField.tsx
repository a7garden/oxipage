import { useRef, useState } from "react";
import { uploadImage } from "../api";
import { adminAssetResolver } from "../../../shared/assets";
import { Input } from "../../../shared/ui/input";
import { Button } from "../../../shared/ui/button";

interface ImageFieldProps {
  slug: string;
  /** Extension namespace (e.g. "profile", "novels"). */
  extension: string;
  /** Current stored value — either a logical path (`media/...`) or an absolute URL. */
  value: string | null;
  /** Called with the new value (logical path or absolute URL). */
  onChange: (next: string | null) => void;
  /** Optional MIME-type filter passed to the file input. */
  accept?: string;
  /** Optional label rendered above the field. */
  label?: string;
  /** Disabled state propagates to input + upload + clear. */
  disabled?: boolean;
}

export function ImageField({
  slug,
  extension,
  value,
  onChange,
  accept = "image/png,image/jpeg,image/webp,image/gif",
  label,
  disabled,
}: ImageFieldProps) {
  const [urlInput, setUrlInput] = useState(value ?? "");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const resolver = adminAssetResolver(slug);
  const previewSrc = resolver.resolve(value);

  function apply(next: string | null) {
    setError(null);
    onChange(next);
    if (next === null) setUrlInput("");
  }

  async function onFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    setPending(true);
    setError(null);
    try {
      const media = await uploadImage(slug, extension, file);
      apply(media.path);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Upload failed");
    } finally {
      setPending(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  }

  function onUrlBlur() {
    const trimmed = urlInput.trim();
    if (trimmed === (value ?? "")) return;
    apply(trimmed === "" ? null : trimmed);
  }

  function onClear() {
    apply(null);
  }

  return (
    <div className="space-y-2">
      {label && (
        <div className="text-sm font-medium text-foreground">{label}</div>
      )}
      <div className="flex gap-2 items-start">
        <div className="w-24 h-24 rounded-md border border-line bg-surface/40 flex items-center justify-center overflow-hidden shrink-0">
          {previewSrc ? (
            // The src is admin-resolved; never trust the raw stored value.
            <img
              src={previewSrc}
              alt=""
              className="w-full h-full object-cover"
              onError={() => setError("Image failed to load")}
            />
          ) : (
            <span className="text-xs text-muted">No image</span>
          )}
        </div>
        <div className="flex-1 space-y-2">
          <Input
            type="url"
            value={urlInput}
            onChange={(e) => setUrlInput(e.target.value)}
            onBlur={onUrlBlur}
            placeholder="https://example.com/image.png or media/profile/..."
            disabled={disabled}
          />
          <div className="flex gap-2">
            <input
              ref={fileRef}
              type="file"
              accept={accept}
              className="hidden"
              onChange={onFile}
              disabled={disabled || pending}
            />
            <Button
              type="button"
              variant="outline"
              onClick={() => fileRef.current?.click()}
              disabled={disabled || pending}
            >
              {pending ? "Uploading…" : "Upload"}
            </Button>
            <Button
              type="button"
              variant="ghost"
              onClick={onClear}
              disabled={disabled || !value}
            >
              Clear
            </Button>
          </div>
          {error && (
            <p className="text-xs text-red-500" role="alert">
              {error}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
