import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { useEffect, useState } from 'react';
import { fetchManifest } from '../shared/api';
import { useLanguage } from '../shared/language';
import './lobby.css';

type DisplayMode = 'canvas' | 'grid' | 'list';

function prefersReducedMotion(): boolean {
  if (typeof window === 'undefined' || !window.matchMedia) return false;
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

export function Lobby() {
  const { data: manifest } = useQuery({ queryKey: ['manifest'], queryFn: fetchManifest });
  const { lang } = useLanguage();
  const [reduced, setReduced] = useState(prefersReducedMotion);

  useEffect(() => {
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    const handler = () => setReduced(mq.matches);
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, []);

  if (!manifest) return null;

  // doc/03 §3.6 — 접근성: reduced-motion 시 canvas → grid 강제 폴백.
  const effectiveMode = (mode: DisplayMode): DisplayMode =>
    reduced && mode === 'canvas' ? 'grid' : mode;

  // display_order 순 정렬.
  const exts = [...manifest.extensions].sort(
    (a, b) => a.lobby.display_order - b.lobby.display_order,
  );

  return (
    <section className="lobby" data-default-mode={manifest.site.default_lang ? 'grid' : 'grid'}>
      {exts.map((ext) => {
        const mode = effectiveMode(ext.lobby.display_mode);
        const name = (lang === 'ko' ? ext.display_name.ko : ext.display_name.en) ?? ext.id;
        return (
          <Link
            key={ext.id}
            to={`/${ext.id}`}
            className={`lobby-item mode-${mode}`}
            data-extension={ext.id}
          >
            <h2>{name}</h2>
          </Link>
        );
      })}
    </section>
  );
}
