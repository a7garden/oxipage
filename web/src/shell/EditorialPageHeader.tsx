import { ArrowLeft } from "lucide-react";
import { Link } from "react-router";

import { useLanguage } from "../shared/language";

/**
 * Per-page header used by every editorial-layout page.
 *
 * - Back link to "/" (the hub)
 * - H1 (page title)
 * - Optional count badge (e.g. total posts)
 * - Optional stats link (e.g. /activity)
 */
export function EditorialPageHeader({
  title,
  count,
  statsHref,
}: {
  title: string;
  count?: number;
  statsHref?: string;
}) {
  const { lang } = useLanguage();

  return (
    <header className="mb-8 flex items-end justify-between gap-4 border-b border-line pb-4">
      <div className="flex items-center gap-3">
        <Link
          to="/"
          className="text-subtle transition-colors hover:text-foreground"
          aria-label={lang === "ko" ? "뒤로" : "Back"}
        >
          <ArrowLeft className="size-5" />
        </Link>
        <h1 className="font-serif text-2xl font-semibold tracking-tight text-foreground">
          {title}
        </h1>
      </div>
      <div className="flex items-center gap-4 text-sm text-subtle">
        {count != null && <span aria-label="count">{count}</span>}
        {statsHref && (
          <Link
            to={statsHref}
            className="transition-colors hover:text-foreground"
          >
            {lang === "ko" ? "통계" : "Stats"}
          </Link>
        )}
      </div>
    </header>
  );
}
