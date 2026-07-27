import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query';
import { BrowserRouter, Link, Route, Routes } from 'react-router';
import { lazy, Suspense } from 'react';
import { fetchManifest } from './shared/api';
import { Lobby } from './lobby/Lobby';
import { ProfilePage } from './extensions/profile/ProfilePage';
import { LanguageProvider, useLanguage } from './shared/language';
import type { Lang } from './shared/language';
import { ThemeToggle } from './shared/ThemeToggle';

const queryClient = new QueryClient();

// Lazy chunks — 확장 하나 = lazy route chunk 하나 (doc/01 §1.5 코드 스플리팅).
const BlogListPage = lazy(() =>
  import('./extensions/blog/BlogListPage').then((m) => ({ default: m.BlogListPage })),
);
const BlogPostPage = lazy(() =>
  import('./extensions/blog/BlogPostPage').then((m) => ({ default: m.BlogPostPage })),
);
const ProjectsListPage = lazy(() =>
  import('./extensions/projects/ProjectsListPage').then((m) => ({ default: m.ProjectsListPage })),
);
const ProjectDetailPage = lazy(() =>
  import('./extensions/projects/ProjectDetailPage').then((m) => ({ default: m.ProjectDetailPage })),
);
const LinksPage = lazy(() =>
  import('./extensions/links/LinksPage').then((m) => ({ default: m.LinksPage })),
);
const SearchPage = lazy(() =>
  import('./search/SearchPage').then((m) => ({ default: m.SearchPage })),
);

function LangToggle() {
  const { lang, setLang } = useLanguage();
  return (
    <button
      type="button"
      className="theme-toggle"
      aria-label="언어 전환 / Switch language"
      onClick={() => setLang(lang === 'ko' ? 'en' : 'ko')}
    >
      {lang === 'ko' ? 'EN' : 'KO'}
    </button>
  );
}

function PageFallback() {
  return <p className="text-tertiary">…</p>;
}

function Shell() {
  const { data: manifest } = useQuery({ queryKey: ['manifest'], queryFn: fetchManifest });
  const defaultLang: Lang = manifest?.site.default_lang === 'en' ? 'en' : 'ko';

  return (
    <LanguageProvider key={defaultLang} defaultLang={defaultLang}>
      <div className="app-shell">
        <header className="app-header">
          <Link to="/" className="site-name">
            {manifest?.site.name ?? 'Oxipage'}
          </Link>
          <div className="header-actions">
            <Link to="/search" className="header-link" aria-label="검색 / Search">
              {defaultLang === 'en' ? 'Search' : '검색'}
            </Link>
            <LangToggle />
            <ThemeToggle />
          </div>
        </header>
        <main>
          <Routes>
            <Route path="/" element={<Lobby />} />
            <Route path="/profile" element={<ProfilePage />} />
            <Route
              path="/blog"
              element={<Suspense fallback={<PageFallback />}><BlogListPage /></Suspense>}
            />
            <Route
              path="/blog/:slug"
              element={<Suspense fallback={<PageFallback />}><BlogPostPage /></Suspense>}
            />
            <Route
              path="/projects"
              element={<Suspense fallback={<PageFallback />}><ProjectsListPage /></Suspense>}
            />
            <Route
              path="/projects/:slug"
              element={<Suspense fallback={<PageFallback />}><ProjectDetailPage /></Suspense>}
            />
            <Route
              path="/links"
              element={<Suspense fallback={<PageFallback />}><LinksPage /></Suspense>}
            />
            <Route
              path="/search"
              element={<Suspense fallback={<PageFallback />}><SearchPage /></Suspense>}
            />
            <Route path="*" element={<p className="text-tertiary">404</p>} />
          </Routes>
        </main>
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
