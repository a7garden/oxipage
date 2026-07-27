import { useQuery } from '@tanstack/react-query';
import { fetchProfile } from '../../shared/api';
import { useLanguage } from '../../shared/language';
import { Markdown } from '../../shared/Markdown';
import './profile.css';

export function ProfilePage() {
  const { data: profile, isLoading, error } = useQuery({
    queryKey: ['profile'],
    queryFn: fetchProfile,
  });
  const { pick, lang } = useLanguage();

  if (isLoading) return <p className="text-tertiary">…</p>;
  if (error || !profile) return <p className="text-tertiary">프로필을 불러오지 못했습니다.</p>;

  const tagline = pick(profile.tagline_ko, profile.tagline_en);
  const bio = pick(profile.bio_ko, profile.bio_en);

  return (
    <article>
      <div className="card profile-hero">
        {profile.avatar_url && (
          <img className="profile-avatar" src={profile.avatar_url} alt={profile.display_name} />
        )}
        <div>
          <h1>{profile.display_name}</h1>
          {tagline && <p className="profile-tagline text-secondary">{tagline}</p>}
          <nav className="profile-contacts">
            {profile.email && <a href={`mailto:${profile.email}`}>{profile.email}</a>}
            {profile.github_username && (
              <a href={`https://github.com/${profile.github_username}`} rel="me">
                GitHub
              </a>
            )}
            {profile.linkedin_url && <a href={profile.linkedin_url}>LinkedIn</a>}
            {profile.custom_links.map((l) => (
              <a key={l.url} href={l.url}>
                {l.label}
              </a>
            ))}
          </nav>
        </div>
      </div>

      {bio && (
        <section className="profile-section card">
          <Markdown source={bio} />
        </section>
      )}

      {profile.education.length > 0 && (
        <section className="profile-section">
          <h2>{lang === 'ko' ? '학력' : 'Education'}</h2>
          <ul className="profile-education">
            {profile.education.map((e, i) => (
              <li key={i} className="card">
                <strong>{e.institution}</strong>
                {(e.degree || e.field) && (
                  <span className="text-secondary">
                    {' '}
                    — {[e.degree, e.field].filter(Boolean).join(', ')}
                  </span>
                )}
                {(e.start_year || e.end_year) && (
                  <span className="text-tertiary">
                    {' '}
                    ({e.start_year ?? '?'}–{e.end_year ?? (lang === 'ko' ? '현재' : 'present')})
                  </span>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}
    </article>
  );
}
