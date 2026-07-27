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

export interface ManifestExtension {
  id: string;
  display_name: LocalizedName;
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
