import { useQuery } from '@tanstack/react-query';
import { Link, useParams } from 'react-router';
import { fetchBlogPost } from '../../shared/api';
import { useLanguage } from '../../shared/language';
import { Markdown } from '../../shared/Markdown';
import './blog.css';

export function BlogPostPage() {
  const { slug = '' } = useParams();
  const { lang } = useLanguage();
  const { data: post, isLoading, error } = useQuery({
    queryKey: ['blog', slug],
    queryFn: () => fetchBlogPost(slug),
    enabled: !!slug,
  });

  if (isLoading) return <p className="text-tertiary">…</p>;
  if (error || !post) {
    return (
      <p className="text-tertiary">
        {lang === 'ko' ? '게시물을 찾을 수 없습니다.' : 'Post not found.'}{' '}
        <Link to="/blog">←</Link>
      </p>
    );
  }

  return (
    <article>
      <Link to="/blog" className="back-link text-tertiary">
        ← {lang === 'ko' ? '블로그' : 'Blog'}
      </Link>
      <header className="card blog-post-header">
        <h1>{post.title}</h1>
        <div className="text-tertiary blog-meta">
          <time>{(post.published_at ?? post.created_at).slice(0, 10)}</time>
          <span>{post.lang === 'ko' ? '한국어' : 'English'}</span>
          {post.tags.length > 0 && <span className="blog-tags">{post.tags.join(' · ')}</span>}
        </div>
      </header>
      <section className="card markdown-container">
        <Markdown source={post.body || `*${lang === 'ko' ? '내용 없음' : 'No content'}*`} />
      </section>
    </article>
  );
}
