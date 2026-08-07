import { useState, type KeyboardEvent } from "react";
import { X } from "lucide-react";

import { cn } from "../../../shared/ui/cn";

interface TagInputProps {
  value: string[];
  onChange: (tags: string[]) => void;
  placeholder?: string;
  error?: string;
}

/// Chip-based tag editor. Enter or comma adds; Backspace on empty input
/// removes the last tag. onChange returns the new array; never mutates `value`.
export function TagInput({
  value,
  onChange,
  placeholder = "Add a tag…",
  error,
}: TagInputProps) {
  const [draft, setDraft] = useState("");

  const commit = (raw: string) => {
    const tag = raw.trim();
    if (!tag || value.includes(tag)) return;
    onChange([...value, tag]);
    setDraft("");
  };

  const remove = (tag: string) => onChange(value.filter((t) => t !== tag));

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      commit(draft);
    } else if (e.key === "Backspace" && draft === "" && value.length > 0) {
      e.preventDefault();
      onChange(value.slice(0, -1));
    }
  };

  return (
    <div>
      <div
        className={cn(
          "flex min-h-10 flex-wrap items-center gap-1.5 rounded-md border bg-canvas px-2 py-1.5",
          error ? "border-destructive-border" : "border-line",
        )}
      >
        {value.map((tag) => (
          <span
            key={tag}
            className="inline-flex items-center gap-1 rounded-md bg-surface px-2 py-0.5 text-xs text-foreground"
          >
            {tag}
            <button
              type="button"
              onClick={() => remove(tag)}
              aria-label={`Remove ${tag}`}
              className="text-muted hover:text-foreground"
            >
              <X className="size-3" />
            </button>
          </span>
        ))}
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={onKeyDown}
          onBlur={() => commit(draft)}
          placeholder={value.length === 0 ? placeholder : ""}
          className="flex-1 min-w-[8rem] bg-transparent text-sm text-foreground outline-none placeholder:text-muted"
        />
      </div>
      {error && <p className="text-xs text-destructive-fg mt-1">{error}</p>}
    </div>
  );
}