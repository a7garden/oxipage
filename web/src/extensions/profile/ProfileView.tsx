import { Briefcase, Code, Globe, Mail } from "lucide-react";

import { Markdown } from "../../shared/Markdown";
import { Card, CardContent } from "../../shared/ui/card";

export interface ProfileEducation {
  institution: string | null;
  degree: string | null;
  field: string | null;
  start_year: number | null;
  end_year: number | null;
}

export interface ProfileCustomLink {
  label: string;
  url: string;
  icon?: string | null;
}

export interface ProfileData {
  display_name: string;
  tagline_ko: string | null;
  tagline_en: string | null;
  avatar_url: string | null;
  bio_ko: string | null;
  bio_en: string | null;
  email: string | null;
  github_username: string | null;
  linkedin_url: string | null;
  education: ProfileEducation[];
  custom_links: ProfileCustomLink[];
  updated_at: string;
}

interface ProfileViewProps {
  profile: ProfileData;
  language: "ko" | "en";
}

function pick<T>(lang: "ko" | "en", ko: T | null, en: T | null): T | null {
  return lang === "ko" ? ko ?? en : en ?? ko;
}

export function ProfileView({ profile, language }: ProfileViewProps) {
  const tagline = pick(language, profile.tagline_ko, profile.tagline_en);
  const bio = pick(language, profile.bio_ko, profile.bio_en);
  const avatarUrl = profile.avatar_url;

  return (
    <article className="space-y-6">
      <Card>
        <div className="flex flex-col gap-5 p-6 sm:flex-row sm:items-start">
          {avatarUrl && (
            <img
              src={avatarUrl}
              alt={profile.display_name}
              className="size-20 shrink-0 rounded-full border border-line object-cover"
            />
          )}
          <div className="min-w-0 space-y-1.5">
            <h1 className="font-serif text-2xl font-semibold tracking-tight text-foreground">
              {profile.display_name}
            </h1>
            {tagline && <p className="leading-relaxed text-muted">{tagline}</p>}
            <nav className="flex flex-wrap gap-x-4 gap-y-1.5 pt-2 text-sm">
              {profile.email && (
                <a className="inline-flex items-center gap-1.5 text-muted hover:text-primary" href={`mailto:${profile.email}`}>
                  <Mail className="size-3.5" />
                  {profile.email}
                </a>
              )}
              {profile.github_username && (
                <a className="inline-flex items-center gap-1.5 text-muted hover:text-primary" href={`https://github.com/${profile.github_username}`} rel="me">
                  <Code className="size-3.5" />
                  GitHub
                </a>
              )}
              {profile.linkedin_url && (
                <a className="inline-flex items-center gap-1.5 text-muted hover:text-primary" href={profile.linkedin_url}>
                  <Briefcase className="size-3.5" />
                  LinkedIn
                </a>
              )}
              {profile.custom_links.map((l) => (
                <a key={l.url} className="inline-flex items-center gap-1.5 text-muted hover:text-primary" href={l.url}>
                  <Globe className="size-3.5" />
                  {l.label}
                </a>
              ))}
            </nav>
          </div>
        </div>
      </Card>

      {bio && (
        <Card>
          <CardContent className="markdown pt-6">
            <Markdown source={bio} />
          </CardContent>
        </Card>
      )}

      {profile.education.length > 0 && (
        <section className="space-y-3">
          <h2 className="font-serif text-xl font-semibold tracking-tight text-foreground">
            {language === "ko" ? "학력" : "Education"}
          </h2>
          <ul className="space-y-2">
            {profile.education.map((e, i) => (
              <li key={i}>
                <Card className="px-4 py-3 shadow-xs">
                  <span className="font-medium text-foreground">{e.institution}</span>
                  {(e.degree || e.field) && (
                    <span className="text-muted">
                      {" "}— {[e.degree, e.field].filter(Boolean).join(", ")}
                    </span>
                  )}
                  {(e.start_year || e.end_year) && (
                    <span className="text-subtle">
                      {" "}({e.start_year ?? "?"}–{e.end_year ?? (language === "ko" ? "현재" : "present")})
                    </span>
                  )}
                </Card>
              </li>
            ))}
          </ul>
        </section>
      )}
    </article>
  );
}