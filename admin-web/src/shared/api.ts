// Admin API client. All calls are same-origin (no token exposure).
// Proxy calls flow through: /api/admin/proxy/{site}/api/console/...

import { createContext, useContext } from "react";

// ─── Types ───

export interface SiteProfile {
  name: string;
  endpoint: string;
  token_masked: string | null;
  active: boolean;
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

// ─── Active Site Context ───

export interface SiteContextValue {
  activeSite: SiteProfile | null;
  setActiveSite: (name: string) => Promise<void>;
  sites: SiteProfile[];
  refreshSites: () => Promise<void>;
}

export const SiteContext = createContext<SiteContextValue>({
  activeSite: null,
  setActiveSite: async () => {},
  sites: [],
  refreshSites: async () => {},
});

export function useSite() {
  return useContext(SiteContext);
}

// ─── Direct Admin API ───

const BASE = "/api/admin";

export async function listSites(): Promise<{ data: SiteProfile[] }> {
  const r = await fetch(`${BASE}/sites`);
  if (!r.ok) throw new Error(`listSites: ${r.status}`);
  return r.json();
}

export async function addSite(name: string, endpoint: string, token?: string): Promise<void> {
  const r = await fetch(`${BASE}/sites`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name, endpoint, token }),
  });
  if (!r.ok) throw new Error(`addSite: ${r.status}`);
}

export async function deleteSite(name: string): Promise<void> {
  const r = await fetch(`${BASE}/sites/${encodeURIComponent(name)}`, { method: "DELETE" });
  if (!r.ok) throw new Error(`deleteSite: ${r.status}`);
}

export async function setActiveSite(name: string): Promise<void> {
  const r = await fetch(`${BASE}/sites/active`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
  if (!r.ok) throw new Error(`setActiveSite: ${r.status}`);
}

export async function getActiveSite(): Promise<{ data: { name: string | null } }> {
  const r = await fetch(`${BASE}/sites/active`);
  if (!r.ok) throw new Error(`getActiveSite: ${r.status}`);
  return r.json();
}

export async function getThemeCatalog(): Promise<{ data: ThemeInfo[] }> {
  const r = await fetch(`${BASE}/themes`);
  if (!r.ok) throw new Error(`getThemeCatalog: ${r.status}`);
  return r.json();
}

// ─── Proxy API (site-scoped) ───

function sitePath(siteName: string, path: string): string {
  const clean = path.startsWith("/") ? path.slice(1) : path;
  return `${BASE}/proxy/${encodeURIComponent(siteName)}/${clean}`;
}

export async function siteGet<T>(siteName: string, path: string): Promise<T> {
  const r = await fetch(sitePath(siteName, path));
  if (!r.ok) {
    const body = await r.text().catch(() => "");
    throw new Error(`GET ${path}: ${r.status} ${body}`);
  }
  return r.json();
}

export async function sitePost<T>(siteName: string, path: string, body?: unknown): Promise<T> {
  const r = await fetch(sitePath(siteName, path), {
    method: "POST",
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!r.ok) {
    const text = await r.text().catch(() => "");
    throw new Error(`POST ${path}: ${r.status} ${text}`);
  }
  return r.json();
}

export async function sitePut<T>(siteName: string, path: string, body?: unknown): Promise<T> {
  const r = await fetch(sitePath(siteName, path), {
    method: "PUT",
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!r.ok) {
    const text = await r.text().catch(() => "");
    throw new Error(`PUT ${path}: ${r.status} ${text}`);
  }
  return r.json();
}

export async function siteDelete(siteName: string, path: string): Promise<void> {
  const r = await fetch(sitePath(siteName, path), { method: "DELETE" });
  if (!r.ok) {
    const text = await r.text().catch(() => "");
    throw new Error(`DELETE ${path}: ${r.status} ${text}`);
  }
}

// ─── Blog-specific API helpers (proxy-scoped) ───

export interface BlogPost {
  id: number;
  slug: string;
  title: string;
  body: string;
  lang: string;
  tags: string[];
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface BlogPostInput {
  title: string;
  body?: string;
  lang?: string;
  tags?: string[];
  slug?: string;
}

export interface BlogPatch {
  title?: string;
  body?: string;
  lang?: string;
  tags?: string[];
}

export async function listBlogPosts(site: string, draft?: boolean): Promise<{ data: BlogPost[] }> {
  const q = draft === undefined ? "" : `?draft=${draft}`;
  return siteGet(site, `api/v1/blog${q}`);
}

export async function getBlogPost(site: string, slug: string): Promise<{ data: BlogPost }> {
  return siteGet(site, `api/v1/blog/${encodeURIComponent(slug)}`);
}

export async function createBlogPost(site: string, input: BlogPostInput): Promise<{ data: BlogPost }> {
  return sitePost(site, "api/v1/blog", input);
}

export async function updateBlogPost(site: string, slug: string, patch: BlogPatch): Promise<{ data: BlogPost }> {
  const r = await fetch(sitePath(site, `api/v1/blog/${encodeURIComponent(slug)}`), {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(patch),
  });
  if (!r.ok) throw new Error(`PATCH blog/${slug}: ${r.status}`);
  return r.json();
}

export async function deleteBlogPost(site: string, slug: string): Promise<void> {
  return siteDelete(site, `api/v1/blog/${encodeURIComponent(slug)}`);
}

export async function publishBlogPost(site: string, slug: string): Promise<{ data: BlogPost }> {
  return sitePost(site, `api/v1/blog/${encodeURIComponent(slug)}/publish`);
}

// ─── Tokens ───

export interface PatRow {
  id: number;
  label: string;
  scopes: string;
  created_at: string;
  expires_at: string | null;
}

export async function listTokens(site: string): Promise<{ data: PatRow[] }> {
  return siteGet(site, "api/v1/auth/tokens");
}

export async function createToken(site: string, label: string, scopes?: string): Promise<{ data: { id: number; label: string; token: string } }> {
  return sitePost(site, "api/v1/auth/tokens", { label, scopes: scopes || "admin" });
}

export async function revokeToken(site: string, id: number): Promise<void> {
  return siteDelete(site, `api/v1/auth/tokens/${id}`);
}
