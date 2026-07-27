import { createContext, useContext, useMemo, useState, type ReactNode } from 'react';

export type Lang = 'ko' | 'en';

interface LanguageValue {
  lang: Lang;
  setLang: (l: Lang) => void;
  pick: (ko?: string | null, en?: string | null) => string;
}

const LanguageContext = createContext<LanguageValue | null>(null);

export function LanguageProvider({
  defaultLang,
  children,
}: {
  defaultLang: Lang;
  children: ReactNode;
}) {
  const [lang, setLang] = useState<Lang>(defaultLang);
  const value = useMemo<LanguageValue>(
    () => ({
      lang,
      setLang,
      pick: (ko, en) => (lang === 'ko' ? ko || en || '' : en || ko || ''),
    }),
    [lang],
  );
  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}

export function useLanguage(): LanguageValue {
  const ctx = useContext(LanguageContext);
  if (!ctx) throw new Error('useLanguage must be used within LanguageProvider');
  return ctx;
}
