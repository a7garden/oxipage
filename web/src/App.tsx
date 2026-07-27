import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { BrowserRouter, Link, Route, Routes } from "react-router";
import { lazy, Suspense } from "react";
import { Languages, Search } from "lucide-react";

import { fetchManifest } from "./shared/api";
import { Lobby } from "./lobby/Lobby";
import { ProfilePage } from "./extensions/profile/ProfilePage";
import { LanguageProvider, useLanguage } from "./shared/language";
import type { Lang } from "./shared/language";
import { ThemeToggle } from "./shared/ThemeToggle";
import { Button } from "./shared/ui/button";
import { Container } from "./shared/ui/container";
import { Skeleton } from "./shared/ui/skeleton";

const queryClient = new QueryClient();

// Lazy chunks — 확장 하나 = lazy route chunk 하나 (doc/01 §1.5 코드 스플리팅).
const BlogListPage = lazy(() =>
  import("./extensions/blog/BlogListPage").then((m) => ({ default: m.BlogListPage })),
);
const BlogPostPage = lazy(() =>
  import("./extensions/blog/BlogPostPage").then((m) => ({ default: m.BlogPostPage })),
);
const ProjectsListPage = lazy(() =>
  import("./extensions/projects/ProjectsListPage").then((m) => ({ default: m.ProjectsListPage })),
);
const ProjectDetailPage = lazy(() =>
  import("./extensions/projects/ProjectDetailPage").then((m) => ({ default: m.ProjectDetailPage })),
);
const LinksPage = lazy(() =>
  import("./extensions/links/LinksPage").then((m) => ({ default: m.LinksPage })),
);
const SearchPage = lazy(() =>
  import("./search/SearchPage").then((m) => ({ default: m.SearchPage })),
);

function LangToggle() {
  const { lang, setLang } = useLanguage();
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      aria-label="언어 전환 / Switch language"
      onClick={() => setLang(lang === "ko" ? "en" : "ko")}
    >
      <Languages />
      {lang === "ko" ? "EN" : "KO"}
    </Button>
  );
}

function PageFallback() {
  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {Array.from({ length: 6 }).map((_, i) => (
        <Skeleton key={i} className="h-28" />
      ))}
    </div>
  );
}

function Shell() {
  const { data: manifest } = useQuery({ queryKey: ["manifest"], queryFn: fetchManifest });
  const defaultLang: Lang = manifest?.site.default_lang === "en" ? "en" : "ko";
  const siteName = manifest?.site.name ?? "Oxipage";

  return (
    <LanguageProvider key={defaultLang} defaultLang={defaultLang}>
      <div className="flex min-h-screen flex-col">
        <header className="bg-canvas/80 supports-[backdrop-filter]:bg-canvas/60 sticky top-0 z-40 border-b border-line backdrop-blur-md">
          <Container className="flex h-14 items-center justify-between gap-4">
            <Link
              to="/"
              className="font-serif text-lg font-semibold tracking-tight text-foreground transition-colors hover:text-primary"
            >
              {siteName}
            </Link>
            <nav className="flex items-center gap-1">
              <Button variant="ghost" size="sm" asChild>
                <Link to="/search">
                  <Search />
                  <span className="hidden sm:inline">
                    {defaultLang === "en" ? "Search" : "검색"}
                  </span>
                </Link>
              </Button>
              <LangToggle />
              <ThemeToggle />
            </nav>
          </Container>
        </header>

        <main className="flex-1 py-8">
          <Container>
            <Routes>
              <Route path="/" element={<Lobby />} />
              <Route path="/profile" element={<ProfilePage />} />
              <Route
                path="/blog"
                element={
                  <Suspense fallback={<PageFallback />}>
                    <BlogListPage />
                  </Suspense>
                }
              />
              <Route
                path="/blog/:slug"
                element={
                  <Suspense fallback={<PageFallback />}>
                    <BlogPostPage />
                  </Suspense>
                }
              />
              <Route
                path="/projects"
                element={
                  <Suspense fallback={<PageFallback />}>
                    <ProjectsListPage />
                  </Suspense>
                }
              />
              <Route
                path="/projects/:slug"
                element={
                  <Suspense fallback={<PageFallback />}>
                    <ProjectDetailPage />
                  </Suspense>
                }
              />
              <Route
                path="/links"
                element={
                  <Suspense fallback={<PageFallback />}>
                    <LinksPage />
                  </Suspense>
                }
              />
              <Route
                path="/search"
                element={
                  <Suspense fallback={<PageFallback />}>
                    <SearchPage />
                  </Suspense>
                }
              />
              <Route path="*" element={<p className="text-subtle">404</p>} />
            </Routes>
          </Container>
        </main>

        <footer className="border-t border-line py-6">
          <Container>
            <p className="text-center text-sm text-subtle">
              {siteName} · Oxipage
            </p>
          </Container>
        </footer>
      </div>
    </LanguageProvider>
  );
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Shell />
      </BrowserRouter>
    </QueryClientProvider>
  );
}
