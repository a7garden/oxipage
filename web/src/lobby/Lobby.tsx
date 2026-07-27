import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { fetchManifest } from '../shared/api';
import { useLanguage } from '../shared/language';
import './lobby.css';

export function Lobby() {
  const { data: manifest } = useQuery({ queryKey: ['manifest'], queryFn: fetchManifest });
  const { lang } = useLanguage();

  if (!manifest) return null;

  return (
    <section className="lobby-grid">
      {manifest.extensions.map((ext) => (
        <Link key={ext.id} to={`/${ext.id}`} className="card lobby-card">
          <h2>{(lang === 'ko' ? ext.display_name.ko : ext.display_name.en) ?? ext.id}</h2>
        </Link>
      ))}
    </section>
  );
}
