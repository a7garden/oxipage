const CONSOLE_BASE = "/api/console";

// ─── Sites ────────────────────────────────────────────────────────────────

export interface SiteInfo {
  name: string;
  path: string;
  active: boolean;
}

export async function listSites(): Promise<{ data: SiteInfo[] }> {
  const res = await fetch(`${CONSOLE_BASE}/sites`);
  return res.json();
}

export async function getDefaultSite(): Promise<{ data: { default_site: string | null } }> {
  const res = await fetch(`${CONSOLE_BASE}/sites/default`);
  return res.json();
}

export interface CreateSiteResult {
  data: { slug: string; path: string };
}

export async function createSite(path: string): Promise<CreateSiteResult> {
  const res = await fetch(`${CONSOLE_BASE}/setup/create-site`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ path }),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new Error(body?.error?.message ?? body?.error ?? `오류 (${res.status})`);
  }
  return res.json();
}

export async function removeSite(slug: string): Promise<Response> {
  return fetch(`${CONSOLE_BASE}/sites/${slug}`, { method: "DELETE" });
}

export async function setDefaultSite(slug: string): Promise<Response> {
  return fetch(`${CONSOLE_BASE}/sites/default`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ default_site: slug }),
  });
}

// ─── Per-site fetch helper ────────────────────────────────────────────────

export async function siteScopedFetch(slug: string, path: string, init?: RequestInit): Promise<Response> {
  return fetch(`${CONSOLE_BASE}/s/${slug}${path}`, init);
}

/// Server validation errors surface as `ApiValidationError`. Carries the
/// offending field so Admin forms can attach the message to the matching
/// DrawerField. Other failures (network, 500) keep their plain Error shape.
export class ApiValidationError extends Error {
  code: string;
  field: string;
  constructor(code: string, field: string, message: string) {
    super(message);
    this.name = "ApiValidationError";
    this.code = code;
    this.field = field;
  }
}

export async function jsonOrThrow<T>(res: Response): Promise<T> {
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    const detail = body?.error;
    if (detail?.field) {
      throw new ApiValidationError(
        detail.code ?? "validation_error",
        detail.field,
        detail.message ?? "Validation failed",
      );
    }
    const msg = detail?.message ?? detail ?? `HTTP ${res.status}`;
    throw new Error(msg);
  }
  return res.json();
}

// ─── Build / Deploy ───────────────────────────────────────────────────────

export interface BuildStart {
  data: { build_id: string; status: string };
}

/// Thrown when a build/deploy is already in flight for the site (HTTP 409).
/// Carries the in-progress run id so the UI can attach to its live stream.
export class OperationConflictError extends Error {
  kind: "build" | "deploy";
  id: string;
  constructor(kind: "build" | "deploy", id: string) {
    super(`${kind}_in_progress`);
    this.name = "OperationConflictError";
    this.kind = kind;
    this.id = id;
  }
}

export async function triggerBuild(slug: string): Promise<BuildStart> {
  const res = await siteScopedFetch(slug, "/build", { method: "POST" });
  if (res.status === 409) {
    const body = await res.json().catch(() => ({}));
    throw new OperationConflictError(body.kind ?? "build", body.run_id ?? "");
  }
  return jsonOrThrow(res);
}

export async function triggerDeploy(
  slug: string,
): Promise<{ data: { deploy_id: string; status: string } }> {
  const res = await siteScopedFetch(slug, "/deploy", { method: "POST" });
  if (res.status === 409) {
    const body = await res.json().catch(() => ({}));
    throw new OperationConflictError(body.kind ?? "deploy", body.run_id ?? "");
  }
  if (res.status === 424) {
    throw new Error("Build the site first — there is no build output to deploy.");
  }
  return jsonOrThrow(res);
}

export function operationStreamUrl(slug: string, kind: "build" | "deploy", id: string): string {
  return `${CONSOLE_BASE}/s/${encodeURIComponent(slug)}/${kind}/${encodeURIComponent(id)}/stream`;
}

export function buildStreamUrl(slug: string, buildId: string): string {
  return `${CONSOLE_BASE}/s/${slug}/build/${buildId}/stream`;
}

export function deployStreamUrl(slug: string, deployId: string): string {
  return `${CONSOLE_BASE}/s/${slug}/deploy/${deployId}/stream`;
}

export interface BuildRecord {
  id: string;
  status: string;
  created_at: string;
  page_count: number | null;
  out_dir: string | null;
}

export async function listBuilds(slug: string): Promise<{ data: BuildRecord[] }> {
  const res = await siteScopedFetch(slug, "/builds");
  return jsonOrThrow(res);
}

// ─── Config (GET/PUT) ─────────────────────────────────────────────────────

export interface SiteConfig {
  name: string;
  base_url: string;
  default_lang: string;
  languages: string[];
}

export interface ServerConfig {
  host: string;
  port: number;
  data_dir: string;
}

export interface LobbyConfig {
  default_mode: string;
}

export interface ConfigResponse {
  site: SiteConfig;
  server: ServerConfig;
  lobby: LobbyConfig;
  extensions: { enabled: string[] };
  integrations: {
    github_username: string | null;
    tmdb_api_key_env: string | null;
    aladin_ttbkey_env: string | null;
  };
  deploy: { github_pages: GitHubPagesTarget | null };
}

export async function getConfig(slug: string): Promise<ConfigResponse> {
  const res = await siteScopedFetch(slug, "/config");
  const json = await jsonOrThrow<{ data: ConfigResponse }>(res);
  return json.data;
}

export async function updateConfig(
  slug: string,
  patch: {
    site?: Partial<SiteConfig>;
    lobby?: Partial<LobbyConfig>;
    integrations?: Partial<{
      github_username: string | null;
      tmdb_api_key_env: string | null;
      aladin_ttbkey_env: string | null;
    }>;
    deploy?: { github_pages: GitHubPagesTarget | null };
  },
): Promise<ConfigResponse> {
  const res = await siteScopedFetch(slug, "/config", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(patch),
  });
  const json = await jsonOrThrow<{ data: ConfigResponse }>(res);
  return json.data;
}

// ─── Deploy (preflight / history / current) ────────────────────────────────

export interface GitHubPagesTarget {
  owner: string;
  repo: string;
  branch: string;
  pages_url?: string;
  base_path?: string;
}

export interface DeployPreflight {
  configured: boolean;
  gh_installed: boolean;
  authenticated: boolean;
  git_repository: boolean;
  origin_matches: boolean;
  build_compatible: boolean;
  pages_url: string | null;
  base_path: string | null;
  problems: { code: string; message: string; action: string | null }[];
}

export interface DeployRecord {
  id: number;
  run_id: string;
  build_id: string;
  target: string;
  owner: string;
  repo: string;
  branch: string;
  base_path: string;
  status: "running" | "deployed" | "unchanged" | "failed";
  url: string | null;
  commit_sha: string | null;
  error_code: string | null;
  error: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface CurrentOperation {
  kind: "build" | "deploy";
  run_id: string;
  active: boolean;
  started_at: string;
  terminal: Record<string, unknown> | null;
}

export async function getDeployPreflight(slug: string): Promise<DeployPreflight> {
  return (await jsonOrThrow<{ data: DeployPreflight }>(await siteScopedFetch(slug, "/deploy/preflight"))).data;
}

export async function listDeploys(slug: string): Promise<DeployRecord[]> {
  return (await jsonOrThrow<{ data: DeployRecord[] }>(await siteScopedFetch(slug, "/deploys?limit=50"))).data;
}

export async function getCurrentOperation(slug: string): Promise<CurrentOperation | null> {
  return (await jsonOrThrow<{ data: CurrentOperation | null }>(await siteScopedFetch(slug, "/operations/current"))).data;
}

export function previewSiteUrl(slug: string): string {
  return `${CONSOLE_BASE}/preview/${encodeURIComponent(slug)}/`;
}

// ─── Stats / Recent ─────────────────────────────────────────────────────────

export interface BuildStatus {
  status: string;
  started_at: string;
  finished_at?: string;
}

export interface StatsResponse {
  counts: Record<string, number>;
  storage_bytes: number;
  last_build: BuildStatus | null;
  last_deploy: DeployRecord | null;
}

export interface RecentItem {
  ext: string;
  id: number;
  title: string;
  updated_at: string;
  published_at: string | null;
}

export async function getStats(slug: string): Promise<StatsResponse> {
  const res = await siteScopedFetch(slug, "/stats");
  const json = await jsonOrThrow<{ data: StatsResponse }>(res);
  return json.data;
}

export async function getRecent(slug: string, limit = 5): Promise<RecentItem[]> {
  const res = await siteScopedFetch(slug, `/content/recent?limit=${limit}`);
  const json = await jsonOrThrow<{ data: RecentItem[] }>(res);
  return json.data;
}

// ─── Theme (GET/PUT) ──────────────────────────────────────────────────────

import type { ThemeDefinition } from "../../shared/theme";

export interface SiteTheme {
  theme_id: string;
  definition: ThemeDefinition;
}

export async function listThemes(): Promise<ThemeDefinition[]> {
  const res = await fetch(`${CONSOLE_BASE}/themes`);
  const json = await jsonOrThrow<{ data: ThemeDefinition[] }>(res);
  return json.data;
}

export async function getTheme(slug: string): Promise<SiteTheme> {
  const res = await siteScopedFetch(slug, "/theme");
  const json = await jsonOrThrow<{ data: SiteTheme }>(res);
  return json.data;
}

export async function setTheme(slug: string, themeId: string): Promise<SiteTheme> {
  const res = await siteScopedFetch(slug, "/theme", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ theme_id: themeId }),
  });
  const json = await jsonOrThrow<{ data: SiteTheme }>(res);
  return json.data;
}

// ─── Extensions (GET list, enable/disable) ────────────────────────────────

export interface ExtensionStatus {
  id: string;
  display_name: string;
  enabled: boolean;
  purged: boolean;
}

export async function listExtensions(slug: string): Promise<ExtensionStatus[]> {
  const res = await siteScopedFetch(slug, "/extensions");
  const json = await jsonOrThrow<{ data: ExtensionStatus[] }>(res);
  return json.data;
}

export async function setExtensionEnabled(slug: string, id: string, enabled: boolean): Promise<ExtensionStatus> {
  const path = enabled ? `/extensions/${id}/enable` : `/extensions/${id}/disable`;
  const res = await siteScopedFetch(slug, path, { method: "POST" });
  const json = await jsonOrThrow<{ data: ExtensionStatus }>(res);
  return json.data;
}

// ─── Content extension CRUD (generic) ─────────────────────────────────────

async function listExtension<T>(
  slug: string,
  extId: string,
  query?: Record<string, string | number | boolean>,
): Promise<T[]> {
  const qs = query
    ? "?" + new URLSearchParams(
        Object.entries(query).map(([k, v]) => [k, String(v)]),
      ).toString()
    : "";
  const res = await siteScopedFetch(slug, `/${extId}${qs}`);
  if (!res.ok) {
    if (res.status === 404) return [];
    throw new Error(`HTTP ${res.status}`);
  }
  const json = (await res.json()) as { data?: T[] };
  return json.data ?? [];
}

export async function showExtension<T>(slug: string, extId: string, id: string): Promise<T | null> {
  const res = await siteScopedFetch(slug, `/${extId}/${id}`);
  if (!res.ok) return null;
  const json = (await res.json()) as { data?: T };
  return json.data ?? null;
}

async function createExtension<T>(slug: string, extId: string, payload: unknown): Promise<T> {
  const res = await siteScopedFetch(slug, `/${extId}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  return jsonOrThrow<{ data: T }>(res).then((j) => j.data);
}

async function updateExtension<T>(slug: string, extId: string, id: string, patch: unknown): Promise<T> {
  const res = await siteScopedFetch(slug, `/${extId}/${id}`, {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(patch),
  });
  return jsonOrThrow<{ data: T }>(res).then((j) => j.data);
}

async function deleteExtension(slug: string, extId: string, id: string): Promise<void> {
  const res = await siteScopedFetch(slug, `/${extId}/${id}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
}

async function actionExtension<T>(slug: string, extId: string, id: string, action: string): Promise<T> {
  const res = await siteScopedFetch(slug, `/${extId}/${id}/${action}`, { method: "POST" });
  return jsonOrThrow<{ data: T }>(res).then((j) => j.data);
}

export const contentClient = {
  list: listExtension,
  show: showExtension,
  create: createExtension,
  update: updateExtension,
  delete: deleteExtension,
  action: actionExtension,
};

// ─── Sub-resource clients (nested paths, not flat contentClient) ────────

// Novels chapters

export interface NovelChapter {
  id: number;
  novel_id: number;
  chapter_order: number;
  title: string;
  body: string;
  char_count: number;
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

export async function reorderChapters(
  slug: string,
  novelSlug: string,
  ids: number[],
): Promise<NovelChapter[]> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters/order`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ chapter_ids: ids }),
  });
  return jsonOrThrow<{ data: NovelChapter[] }>(res).then((b) => b.data);
}

export async function reorderScreenshots(
  slug: string,
  projectSlug: string,
  ids: number[],
): Promise<Screenshot[]> {
  const res = await siteScopedFetch(slug, `/projects/${projectSlug}/screenshots/order`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ screenshot_ids: ids }),
  });
  return jsonOrThrow<{ data: Screenshot[] }>(res).then((b) => b.data);
}

export interface TmdbSearchResult {
  tmdb_id: number;
  title: string;
  media_type: "movie" | "tv";
  poster_path: string | null;
  release_year: number | null;
}

export async function searchTmdb(slug: string, q: string): Promise<TmdbSearchResult[]> {
  const res = await siteScopedFetch(slug, `/movies/search?q=${encodeURIComponent(q)}`);
  return jsonOrThrow<{ data: TmdbSearchResult[] }>(res).then((b) => b.data);
}

export async function listChapters(slug: string, novelSlug: string, draft = false): Promise<NovelChapter[]> {
  const path = draft ? `/novels/${novelSlug}/chapters/draft` : `/novels/${novelSlug}/chapters`;
  const res = await siteScopedFetch(slug, path);
  if (!res.ok) return [];
  const json = (await res.json()) as { data?: NovelChapter[] };
  return json.data ?? [];
}

export async function createChapter(
  slug: string, novelSlug: string, input: { chapter_order: number; title: string; body?: string },
): Promise<NovelChapter> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify(input),
  });
  return jsonOrThrow<{ data: NovelChapter }>(res).then((j) => j.data);
}

export async function updateChapter(
  slug: string, novelSlug: string, order: number,
  patch: { title?: string; body?: string; chapter_order?: number },
): Promise<NovelChapter> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters/${order}`, {
    method: "PATCH", headers: { "content-type": "application/json" },
    body: JSON.stringify(patch),
  });
  return jsonOrThrow<{ data: NovelChapter }>(res).then((j) => j.data);
}

export async function deleteChapter(slug: string, novelSlug: string, order: number): Promise<void> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters/${order}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
}

export async function publishChapter(slug: string, novelSlug: string, order: number): Promise<NovelChapter> {
  const res = await siteScopedFetch(slug, `/novels/${novelSlug}/chapters/${order}/publish`, { method: "POST" });
  return jsonOrThrow<{ data: NovelChapter }>(res).then((j) => j.data);
}

// Movies series groups

export interface SeriesGroup {
  id: number;
  slug: string;
  title_ko: string | null;
  title_en: string | null;
  cover_image: string | null;
  group_rating: number | null;
  created_at: string;
  updated_at: string;
}

export interface SeriesGroupDetail {
  group: SeriesGroup;
  entries: unknown[]; // MovieEntry[] — type defined locally in MoviesTab
}

export async function listSeries(slug: string): Promise<SeriesGroup[]> {
  const res = await siteScopedFetch(slug, "/movies/series");
  if (!res.ok) return [];
  const json = (await res.json()) as { data?: SeriesGroup[] };
  return json.data ?? [];
}

export async function createSeries(
  slug: string, input: { title_ko?: string; title_en?: string },
): Promise<SeriesGroup> {
  const res = await siteScopedFetch(slug, "/movies/series", {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify(input),
  });
  return jsonOrThrow<{ data: SeriesGroup }>(res).then((j) => j.data);
}

export async function showSeries(slug: string, groupSlug: string): Promise<SeriesGroupDetail | null> {
  const res = await siteScopedFetch(slug, `/movies/series/${groupSlug}`);
  if (!res.ok) return null;
  return jsonOrThrow<{ data: SeriesGroupDetail }>(res).then((j) => j.data);
}

export async function updateSeries(
  slug: string, groupSlug: string,
  patch: Partial<{ title_ko: string; title_en: string; cover_image: string; group_rating: number }>,
): Promise<SeriesGroup> {
  const res = await siteScopedFetch(slug, `/movies/series/${groupSlug}`, {
    method: "PATCH", headers: { "content-type": "application/json" },
    body: JSON.stringify(patch),
  });
  return jsonOrThrow<{ data: SeriesGroup }>(res).then((j) => j.data);
}

export async function deleteSeries(slug: string, groupSlug: string): Promise<void> {
  const res = await siteScopedFetch(slug, `/movies/series/${groupSlug}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
}

// Projects screenshots

export interface Screenshot {
  id: number;
  project_id: number;
  url: string;
  alt_ko: string | null;
  alt_en: string | null;
  display_order: number;
  created_at: string;
}

export async function addScreenshot(
  slug: string, projectSlug: string,
  input: { url: string; alt_ko?: string; alt_en?: string; display_order?: number },
): Promise<Screenshot> {
  const res = await siteScopedFetch(slug, `/projects/${projectSlug}/screenshots`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify(input),
  });
  return jsonOrThrow<{ data: Screenshot }>(res).then((j) => j.data);
}

export async function updateScreenshot(
  slug: string, projectSlug: string, sid: number,
  patch: { alt_ko?: string; alt_en?: string; display_order?: number },
): Promise<Screenshot> {
  const res = await siteScopedFetch(slug, `/projects/${projectSlug}/screenshots/${sid}`, {
    method: "PATCH", headers: { "content-type": "application/json" },
    body: JSON.stringify(patch),
  });
  return jsonOrThrow<{ data: Screenshot }>(res).then((j) => j.data);
}

export async function deleteScreenshot(slug: string, projectSlug: string, sid: number): Promise<void> {
  const res = await siteScopedFetch(slug, `/projects/${projectSlug}/screenshots/${sid}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
}

// WASM registry

export interface RegistryEntry {
  name: string;
  runtime_loadable: boolean;
  installed: boolean;
  source: string;
}

export async function listRegistry(): Promise<RegistryEntry[]> {
  const res = await fetch(`/api/console/extensions/registry`);
  if (!res.ok) return [];
  const json = (await res.json()) as { data?: RegistryEntry[] };
  return json.data ?? [];
}

export async function installExtension(name: string): Promise<{ name: string; activated: boolean; note?: string }> {
  const res = await fetch(`/api/console/extensions/install`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ name }),
  });
  return jsonOrThrow<{ data: { name: string; activated: boolean; note?: string } }>(res).then((j) => j.data);
}

// ─── Media upload ─────────────────────────────────────────────────────────

export interface UploadedMedia {
  path: string;
  mime: string;
  bytes: number;
}

export interface UploadResponse {
  data: UploadedMedia;
}

/**
 * POST a single image file to the site media endpoint. The path component
 * specifies a logical extension namespace (e.g. "profile", "novels"). The
 * server validates by magic bytes and returns a logical path like
 * `media/profile/<uuid>.png` — store that in the content row.
 */
export async function uploadImage(
  slug: string,
  extension: string,
  file: File,
): Promise<UploadedMedia> {
  const form = new FormData();
  form.append("file", file);
  const res = await fetch(
    `${CONSOLE_BASE}/s/${slug}/media/${extension}`,
    { method: "POST", body: form },
  );
  const body = await jsonOrThrow<UploadResponse>(res);
  return body.data;
}

/** Prefix-aware URL for the preview iframe. Opens at the deployed base. */
export function previewUrl(slug: string): string {
  return `${CONSOLE_BASE}/preview/${slug}/`;
}
