import { useState, type FormEvent } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { searchAll } from '../shared/api';
import { useLanguage } from '../shared/language';
import './search.css';

export function SearchPage() {
  const { lang } = useLanguage();
  const [q, setQ] = useState('');
  const [submitted, setSubmitted] = useState('');

  const { data: hits, isFetching } = useQuery({
    queryKey: ['search', submitted],
    queryFn: () => searchAll(submitted, lang),
    enabled: submitted.length > 0,
  });

  function onSubmit(e: FormEvent) {
    e.preventDefault();
    setSubmitted(q.trim());
  }

  // doc_id → 프론트 경로 추정. novels "slug/chapters/N" → /novels/{slug}, 그 외 /{ext}/{doc_id}.
  function docUrl(extId: string, docId: string): string {
    if (docId.includes('/')) {
      return `/${extId}/${docId.split('/')[0]}`;
    }
    return `/${extId}/${docId}`;
  }

  return (
    <article>
      <h1 className="page-title">{lang === 'ko' ? '검색' : 'Search'}</h1>
      <form className="search-form" role="search" onSubmit={onSubmit}>
        <input
          type="search"
          className="search-input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={lang === 'ko' ? '검색어 입력…' : 'Search…'}
          aria-label={lang === 'ko' ? '검색어' : 'Search query'}
          autoFocus
        />
        <button type="submit" className="search-submit">
          {lang === 'ko' ? '검색' : 'Search'}
        </button>
      </form>

      {isFetching && <p className="text-tertiary">…</p>}

      {submitted && !isFetching && hits && hits.length === 0 && (
        <p className="text-tertiary">
          {lang === 'ko' ? '결과가 없습니다.' : 'No results.'}
        </p>
      )}

      {hits && hits.length > 0 && (
        <ul className="search-results">
          {hits.map((h, i) => (
            <li key={`${h.extension_id}-${h.doc_id}-${i}`} className="card search-result">
              <Link to={docUrl(h.extension_id, h.doc_id)}>
                <div className="search-result-head">
                  <span className="search-result-ext text-tertiary">{h.extension_id}</span>
                  <h2>{h.title}</h2>
                </div>
                <p
                  className="search-result-snippet text-secondary"
                  dangerouslySetInnerHTML={{ __html: h.snippet }}
                />
              </Link>
            </li>
          ))}
        </ul>
      )}
    </article>
  );
}
