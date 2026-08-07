import type { ReactNode } from "react";

import { cn } from "../../shared/ui/cn";

/**
 * Console page header — the oxi-brand counterpart to the public site's
 * EditorialPageHeader. Sits inside <ConsoleShell> so it never inherits the
 * editorial column rhythm or the serif display font.
 *
 * Tone contract (intentionally tight):
 *  - sans-serif body (o font-sans), no serif Display
 *  - mid-weight title (text-xl font-semibold tracking-tight)
 *  - muted description, single-line above-board actions area
 *  - hairline border only when actions are present
 *
 * Use the optional `actions` slot for inline page-level controls (rebuild,
 * deploy, search). For long-form prose use content blocks directly.
 */
export function ConsolePageHeader({
  title,
  description,
  actions,
  border = true,
  className,
}: {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  /** Hairline below the title block. Defaults to true; pass false to blend
   *  into a page that already provides its own sectioning. */
  border?: boolean;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-wrap items-start justify-between gap-3 pb-4 mb-6",
        border && "border-b border-line",
        className,
      )}
    >
      <div className="min-w-0">
        <h1 className="font-sans text-xl font-semibold tracking-tight text-foreground">
          {title}
        </h1>
        {description && (
          <p className="text-sm text-muted mt-1">{description}</p>
        )}
      </div>
      {actions && (
        <div className="flex items-center gap-2 shrink-0">{actions}</div>
      )}
    </div>
  );
}
