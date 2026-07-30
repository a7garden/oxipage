import { useState } from "react";
import { marked } from "marked";
import { Textarea } from "../../../shared/ui/textarea";

interface Props {
  value: string;
  onChange: (v: string) => void;
  rows?: number;
  placeholder?: string;
}

export function MarkdownEditor({ value, onChange, rows = 6, placeholder }: Props) {
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  return (
    <div className="border border-line rounded overflow-hidden">
      <div className="flex gap-0 border-b border-line bg-surface/30">
        <button
          onClick={() => setMode("edit")}
          className={`px-3 py-1 text-xs font-medium ${mode === "edit" ? "bg-canvas text-foreground" : "text-muted hover:text-foreground"}`}
        >
          Edit
        </button>
        <button
          onClick={() => setMode("preview")}
          className={`px-3 py-1 text-xs font-medium ${mode === "preview" ? "bg-canvas text-foreground" : "text-muted hover:text-foreground"}`}
        >
          Preview
        </button>
      </div>
      {mode === "edit" ? (
        <Textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          rows={rows}
          placeholder={placeholder}
          className="border-0 rounded-none"
        />
      ) : (
        <div
          className="p-3 text-sm prose prose-sm max-w-none"
          dangerouslySetInnerHTML={{ __html: marked.parse(value || "") }}
        />
      )}
    </div>
  );
}
