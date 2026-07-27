import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query';
import { BrowserRouter, Link, Route, Routes } from 'react-router';
import { Lobby } from './lobby/Lobby';
import { ProfilePage } from './extensions/profile/ProfilePage';
import { fetchManifest } from './shared/api';
import { LanguageProvider, useLanguage, type Lang } from './shared/language';
import { ThemeToggle } from './shared/ThemeToggle';

const queryClient = new QueryClient();

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

function Shell() {
  const { data: manifest } = useQuery({ queryKey: ['manifest'], queryFn: fetchManifest });
  const defaultLang: Lang = manifest?.site.default_lang === 'en' ? 'en' : 'ko';

  return (
    <LanguageProvider defaultLang={defaultLang}>
      <div className="app-shell">
        <header className="app-header">
          <Link to="/" className="site-name">
            {manifest?.site.name ?? 'Oxipage'}
          </Link>
          <div className="header-actions">
            <LangToggle />
            <ThemeToggle />
          </div>
        </header>
        <main>
          <Routes>
            <Route path="/" element={<Lobby />} />
            <Route path="/profile" element={<ProfilePage />} />
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
