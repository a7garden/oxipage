export interface LocalizedName {
  ko?: string;
  en?: string;
}

export interface ManifestSite {
  name: string;
  base_url: string;
  default_lang: string;
  languages: string[];
}

export interface LobbyConfigInfo {
  enabled: boolean;
  display_mode: 'canvas' | 'grid' | 'list';
  display_order: number;
  style_params: Record<string, unknown>;
}

export interface ManifestExtension {
  id: string;
  display_name: LocalizedName;
  lobby: LobbyConfigInfo;
}

export interface Manifest {
  site: ManifestSite;
  extensions: ManifestExtension[];
}

export interface Education {
  institution: string | null;
  degree: string | null;
  field: string | null;
  start_year: number | null;
  end_year: number | null;
}

export interface CustomLink {
  label: string;
  url: string;
  icon: string | null;
}

export interface Profile {
  display_name: string;
  tagline_ko: string | null;
  tagline_en: string | null;
  avatar_url: string | null;
  bio_ko: string | null;
  bio_en: string | null;
  email: string | null;
  github_username: string | null;
  linkedin_url: string | null;
  education: Education[];
  custom_links: CustomLink[];
  updated_at: string;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function apiFetch<T>(path: string): Promise<T> {
  const res = await fetch(`/api/v1${path}`);
  if (!res.ok) {
    throw new ApiError(res.status, `API request failed: ${res.status} ${path}`);
  }
  const json = (await res.json()) as { data: T };
  return json.data;
}

export const fetchManifest = () => apiFetch<Manifest>('/lobby/manifest');
export const fetchProfile = () => apiFetch<Profile>('/profile');

// ─── blog (doc/02 §2.6) ───
export interface BlogPost {
  id: number;
  slug: string;
  title: string;
  body: string;
  lang: 'ko' | 'en';
  translation_group_id: number | null;
  tags: string[];
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

// ─── projects (doc/02 §2.4) ───
export interface Project {
  id: number;
  slug: string;
  title_ko: string | null;
  title_en: string | null;
  description_ko: string | null;
  description_en: string | null;
  tech_stack: string[];
  status: 'active' | 'archived' | 'wip';
  started_at: string | null;
  ended_at: string | null;
  links: Record<string, unknown>;
  featured: boolean;
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface Screenshot {
  id: number;
  project_id: number;
  url: string;
  alt_ko: string | null;
  alt_en: string | null;
  display_order: number;
  created_at: string;
}

export interface ProjectDetail extends Project {
  screenshots: Screenshot[];
}

// ─── links (doc/02 §2.11) ───
export interface LinkCard {
  id: number;
  title: string;
  url: string;
  description_ko: string | null;
  description_en: string | null;
  thumbnail_url: string | null;
  tags: string[];
  display_order: number;
  featured: boolean;
  created_at: string;
  updated_at: string;
}

// ─── search (doc/01 §1.7) ───
export interface SearchHit {
  extension_id: string;
  doc_id: string;
  title: string;
  snippet: string;
  lang: string | null;
  published_at: string | null;
}

export const fetchBlogPosts = () => apiFetch<BlogPost[]>('/blog');
export const fetchBlogPost = (slug: string) => apiFetch<BlogPost>(`/blog/${slug}`);
export const fetchProjects = () => apiFetch<Project[]>('/projects');
export const fetchProject = (slug: string) => apiFetch<ProjectDetail>(`/projects/${slug}`);
export const fetchLinks = () => apiFetch<LinkCard[]>('/links');
export const searchAll = (q: string, lang?: 'ko' | 'en') =>
  apiFetch<SearchHit[]>(`/search?q=${encodeURIComponent(q)}${lang ? `&lang=${lang}` : ''}`);
