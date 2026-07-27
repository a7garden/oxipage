import { useQuery } from '@tanstack/react-query';
import { fetchLinks } from '../../shared/api';
import { useLanguage } from '../../shared/language';
import './links.css';

export function LinksPage() {
  const { pick, lang } = useLanguage();
  const { data: links, isLoading } = useQuery({
    queryKey: ['links', 'list'],
    queryFn: fetchLinks,
  });

  if (isLoading) return <p className="text-tertiary">…</p>;
  if (!links || links.length === 0) {
    return (
      <p className="text-tertiary">
        {lang === 'ko' ? '아직 링크가 없습니다.' : 'No links yet.'}
      </p>
    );
  }

  return (
    <article>
      <h1 className="page-title">{lang === 'ko' ? '링크' : 'Links'}</h1>
      <ul className="links-grid">
        {links.map((l) => (
          <li key={l.id} className={`card link-card${l.featured ? ' featured' : ''}`}>
            <a href={l.url} rel="noreferrer noopener">
              {l.thumbnail_url && (
                <img
                  src={l.thumbnail_url}
                  alt=""
                  className="link-thumbnail"
                  loading="lazy"
                />
              )}
              <div className="link-body">
                <h2>{l.title}</h2>
                {pick(l.description_ko, l.description_en) && (
                  <p className="text-secondary">{pick(l.description_ko, l.description_en)}</p>
                )}
                <span className="text-tertiary link-host">
                  {safeHost(l.url)}
                </span>
              </div>
            </a>
          </li>
        ))}
      </ul>
    </article>
  );
}

function safeHost(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}
