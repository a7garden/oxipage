import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { fetchManifest } from "../shared/api";
import { LanguageProvider } from "../shared/language";
import type { Lang } from "../shared/language";

/**
 * Chromeless editorial shell — no header, nav, or footer.
 * Each page renders its own EditorialPageHeader. The route table is
 * injected as children by the App-level routing (see Task 10).
 *
 * Layout-orthogonal: this only changes chrome, not color theme.
 * Language-orthogonal: this derives default_lang from the site manifest
 * the same way Shell does, so editorial respects the configured
 * default_lang (ko or en) instead of hardcoding ko.
 */
export function EditorialShell({ children }: { children: ReactNode }) {
  const { data: manifest } = useQuery({
    queryKey: ["manifest"],
    queryFn: fetchManifest,
  });
  const defaultLang: Lang =
    manifest?.site.default_lang === "en" ? "en" : "ko";

  return (
    <LanguageProvider key={defaultLang} defaultLang={defaultLang}>
      <main className="mx-auto w-full max-w-5xl px-5 py-10 sm:py-16">
        {children}
      </main>
    </LanguageProvider>
  );
}