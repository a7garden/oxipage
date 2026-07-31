import type { ReactNode } from "react";

/// Wraps a presentation component (e.g. BlogPostView) rendered from local
/// form state. Adds a header that distinguishes "Draft Preview" from
/// "Preview Site" (the last static build of the site).
export function DraftPreviewPane({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-md border border-line bg-canvas">
      <div className="flex items-center justify-between border-b border-line px-4 py-2 text-xs text-muted">
        <span className="font-medium">Draft Preview</span>
        <span>unsaved local state</span>
      </div>
      <div className="p-4">{children}</div>
    </div>
  );
}