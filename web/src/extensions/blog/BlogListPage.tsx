import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { fetchBlogPosts } from '../../shared/api';
import { useLanguage } from '../../shared/language';
import './blog.css';

export function BlogListPage() {
  const { lang } = useLanguage();
  const { data: posts, isLoading } = useQuery({
    queryKey: ['blog', 'list', lang],
    queryFn: () => fetchBlogPosts(),
  });

  if (isLoading) return <p className="text-tertiary">…</p>;
  if (!posts || posts.length === 0) {
    return <p className="text-tertiary">{lang === 'ko' ? '아직 게시물이 없습니다.' : 'No posts yet.'}</p>;
  }

  return (
    <article>
      <h1 className="page-title">{lang === 'ko' ? '블로그' : 'Blog'}</h1>
      <ul className="blog-list">
        {posts.map((p) => (
          <li key={p.slug} className="card blog-item">
            <Link to={`/blog/${p.slug}`}>
              <h2>{p.title}</h2>
              <div className="text-tertiary blog-meta">
                <time>{(p.published_at ?? p.created_at).slice(0, 10)}</time>
                {p.tags.length > 0 && <span className="blog-tags">{p.tags.join(' · ')}</span>}
              </div>
            </Link>
          </li>
        ))}
      </ul>
    </article>
  );
}
