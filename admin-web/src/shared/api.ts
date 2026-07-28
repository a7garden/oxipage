// Admin API client. All calls are same-origin to the local console.
// v2 SSG: no site proxy. The local console exposes /api/console/*.

import { createContext, useContext } from "react";

// ─── Types ───

export interface SiteProfile {
  name: string;
  endpoint: string;
  token?: string | null;
}

export interface ThemeInfo {
  id: string;
  name_ko: string;
  name_en: string;
  mode: string;
  accent_hue: number;
  description_ko: string;
  description_en: string;
  preview_colors: string[];
}

// ─── Active Site Context (display-only in v2) ───
// v2: there's only one local site. The "active site" is just a display
// label. Multi-site config is managed by `oxipage site` CLI.
export interface SiteContextValue {
  activeSite: SiteProfile | null;
  setActiveSite: (name: string | null) => void;
  sites: SiteProfile[];
  refreshSites: () => Promise<void>;
}

export const SiteContext = createContext<SiteContextValue>({
  activeSite: null,
  setActiveSite: () => {},
  sites: [],
  refreshSites: async () => {},
});

export function useSite() {
  return useContext(SiteContext);
}

// ─── Direct Admin API (local console) ───

const ADMIN_BASE = "/api/admin";
const CONSOLE_BASE = "/api/console";

interface FetchOpts {
  method?: "GET" | "POST" | "PUT" | "DELETE";
  body?: unknown;
  signal?: AbortSignal;
}

async function adminFetch<T>(path: string, opts: FetchOpts = {}): Promise<T> {
  const { method = "GET", body, signal } = opts;
  const res = await fetch(`${ADMIN_BASE}${path}`, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
    credentials: "include",
    signal,
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`admin API ${path} failed: ${res.status} ${text}`);
  }
  return (await res.json()) as T;
}

async function consoleFetch<T>(path: string, opts: FetchOpts = {}): Promise<T> {
  const { method = "GET", body, signal } = opts;
  const res = await fetch(`${CONSOLE_BASE}${path}`, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
    credentials: "include",
    signal,
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`console API ${path} failed: ${res.status} ${text}`);
  }
  return (await res.json()) as T;
}

// ─── Admin sites/themes ───

export async function listSites(): Promise<{ data: SiteProfile[] }> {
  return adminFetch("/sites");
}

export async function addSite(name: string, endpoint: string, token?: string): Promise<void> {
  await adminFetch("/sites", { method: "POST", body: { name, endpoint, token } });
}

export async function deleteSite(name: string): Promise<void> {
  await adminFetch(`/sites/${encodeURIComponent(name)}`, { method: "DELETE" });
}

export async function setActiveSite(name: string): Promise<void> {
  await adminFetch("/sites/active", { method: "PUT", body: { name } });
}

export async function getActiveSite(): Promise<{ data: { name: string | null } }> {
  return adminFetch("/sites/active");
}

export async function getThemeCatalog(): Promise<{ data: ThemeInfo[] }> {
  return adminFetch("/themes");
}

// ─── Local console API (was: proxy to remote site) ───
// v2 SSG: no remote sites. Always call the local console.


// ─── Generic console API ───

export async function apiGet<T>(path: string, signal?: AbortSignal): Promise<T> {
  return consoleFetch<T>(path, { signal });
}

export async function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return consoleFetch<T>(path, { method: 'POST', body });
}

export async function apiPut<T>(path: string, body?: unknown): Promise<T> {
  return consoleFetch<T>(path, { method: 'PUT', body });
}

export async function apiDelete(path: string): Promise<void> {
  await consoleFetch(path, { method: 'DELETE' });
}

export async function listBlogPosts(
  draft?: boolean,
  lang?: string,
): Promise<{ data: BlogPost[] }> {
  const q = new URLSearchParams();
  if (draft) q.set("draft", "true");
  if (lang) q.set("lang", lang);
  const qs = q.toString();
  return consoleFetch(`/blog${qs ? `?${qs}` : ""}`);
}

export async function getBlogPost(slug: string): Promise<{ data: BlogPost }> {
  return consoleFetch(`/blog/${encodeURIComponent(slug)}`);
}

export async function createBlogPost(input: BlogPostInput): Promise<{ data: BlogPost }> {
  return consoleFetch("/blog", { method: "POST", body: input });
}

export async function updateBlogPost(
  slug: string,
  patch: BlogPatch,
): Promise<{ data: BlogPost }> {
  return consoleFetch(`/blog/${encodeURIComponent(slug)}`, { method: "PATCH", body: patch });
}

export async function deleteBlogPost(slug: string): Promise<void> {
  await consoleFetch(`/blog/${encodeURIComponent(slug)}`, { method: "DELETE" });
}

export async function publishBlogPost(slug: string): Promise<{ data: BlogPost }> {
  return consoleFetch(`/blog/${encodeURIComponent(slug)}/publish`, { method: "POST" });
}

// ─── Types (blog) ───

export interface BlogPost {
  id: number;
  slug: string;
  title: string;
  body: string;
  lang: string;
  translation_group_id: number | null;
  tags: string[];
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface BlogPostInput {
  title: string;
  body: string;
  lang?: string;
  tags?: string[];
  translation_group_id?: number | null;
  slug?: string;
}

export interface BlogPatch {
  title?: string;
  body?: string;
  lang?: string;
  tags?: string[];
}

// ─── Theme (was proxied) ───

export interface CurrentTheme {
  theme_id: string;
}

export async function getCurrentTheme(): Promise<{ data: CurrentTheme }> {
  return consoleFetch("/theme");
}
