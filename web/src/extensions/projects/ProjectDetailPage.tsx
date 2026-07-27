import { useQuery } from '@tanstack/react-query';
import { Link, useParams } from 'react-router';
import { fetchProject } from '../../shared/api';
import { useLanguage } from '../../shared/language';
import { Markdown } from '../../shared/Markdown';
import './projects.css';

interface ProjectLinks {
  repo?: string;
  demo?: string;
  app_store?: string;
  play_store?: string;
  custom?: { label: string; url: string }[];
}

export function ProjectDetailPage() {
  const { slug = '' } = useParams();
  const { pick, lang } = useLanguage();
  const { data: project, isLoading, error } = useQuery({
    queryKey: ['projects', slug],
    queryFn: () => fetchProject(slug),
    enabled: !!slug,
  });

  if (isLoading) return <p className="text-tertiary">…</p>;
  if (error || !project) {
    return (
      <p className="text-tertiary">
        {lang === 'ko' ? '프로젝트를 찾을 수 없습니다.' : 'Project not found.'}{' '}
        <Link to="/projects">←</Link>
      </p>
    );
  }

  const description = pick(project.description_ko, project.description_en);
  const links = (project.links ?? {}) as ProjectLinks;
  const linkEntries: { label: string; url: string }[] = [
    links.repo && { label: 'Repo', url: links.repo },
    links.demo && { label: 'Demo', url: links.demo },
    links.app_store && { label: 'App Store', url: links.app_store },
    links.play_store && { label: 'Play Store', url: links.play_store },
    ...(links.custom ?? []),
  ].filter((x): x is { label: string; url: string } => !!x);

  return (
    <article>
      <Link to="/projects" className="back-link text-tertiary">
        ← {lang === 'ko' ? '프로젝트' : 'Projects'}
      </Link>
      <header className="card project-header">
        <h1>{pick(project.title_ko, project.title_en)}</h1>
        <div className="text-tertiary project-meta">
          <span data-status={project.status}>{project.status}</span>
          {project.tech_stack.length > 0 && <span> · {project.tech_stack.join(' · ')}</span>}
        </div>
        {linkEntries.length > 0 && (
          <nav className="project-links">
            {linkEntries.map((l) => (
              <a key={l.url} href={l.url} rel="noreferrer noopener">
                {l.label}
              </a>
            ))}
          </nav>
        )}
      </header>

      {description && (
        <section className="card markdown-container">
          <Markdown source={description} />
        </section>
      )}

      {project.screenshots.length > 0 && (
        <section className="project-gallery">
          {project.screenshots.map((s) => (
            <figure key={s.id} className="card project-screenshot">
              <img src={s.url} alt={pick(s.alt_ko, s.alt_en) ?? ''} loading="lazy" />
            </figure>
          ))}
        </section>
      )}
    </article>
  );
}
