import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { fetchProjects } from '../../shared/api';
import { useLanguage } from '../../shared/language';
import './projects.css';

export function ProjectsListPage() {
  const { pick, lang } = useLanguage();
  const { data: projects, isLoading } = useQuery({
    queryKey: ['projects', 'list'],
    queryFn: fetchProjects,
  });

  if (isLoading) return <p className="text-tertiary">…</p>;
  if (!projects || projects.length === 0) {
    return (
      <p className="text-tertiary">
        {lang === 'ko' ? '아직 프로젝트가 없습니다.' : 'No projects yet.'}
      </p>
    );
  }

  return (
    <article>
      <h1 className="page-title">{lang === 'ko' ? '프로젝트' : 'Projects'}</h1>
      <ul className="projects-grid">
        {projects.map((p) => (
          <li key={p.slug} className={`card project-card${p.featured ? ' featured' : ''}`}>
            <Link to={`/projects/${p.slug}`}>
              <h2>{pick(p.title_ko, p.title_en)}</h2>
              <p className="text-secondary project-status" data-status={p.status}>
                {p.status}
              </p>
              {p.tech_stack.length > 0 && (
                <p className="text-tertiary project-tech">{p.tech_stack.join(' · ')}</p>
              )}
            </Link>
          </li>
        ))}
      </ul>
    </article>
  );
}
