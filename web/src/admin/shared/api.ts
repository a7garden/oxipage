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
    body: JSON.stringify({ name: slug }),
  });
}

// ─── Per-site fetch helper ────────────────────────────────────────────────

export async function siteScopedFetch(slug: string, path: string, init?: RequestInit): Promise<Response> {
  return fetch(`${CONSOLE_BASE}/s/${slug}${path}`, init);
}

export async function jsonOrThrow<T>(res: Response): Promise<T> {
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    const msg = body?.error?.message ?? body?.error ?? `HTTP ${res.status}`;
    throw new Error(msg);
  }
  return res.json();
}

// ─── Build / Deploy ───────────────────────────────────────────────────────

export interface BuildResult {
  data: { out_dir: string; page_count: number };
}

export async function triggerBuild(slug: string): Promise<BuildResult> {
  const res = await siteScopedFetch(slug, "/build", { method: "POST" });
  return jsonOrThrow(res);
}

export async function triggerDeploy(slug: string): Promise<{ data: { slug: string; status: string; note: string } }> {
  const res = await siteScopedFetch(slug, "/deploy", { method: "POST" });
  return jsonOrThrow(res);
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

// ─── Theme (GET/PUT) ──────────────────────────────────────────────────────

export async function getTheme(slug: string): Promise<{ theme_id: string }> {
  const res = await siteScopedFetch(slug, "/theme");
  const json = await jsonOrThrow<{ data: { theme_id: string } }>(res);
  return json.data;
}

export async function setTheme(slug: string, themeId: string): Promise<{ theme_id: string }> {
  const res = await siteScopedFetch(slug, "/theme", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ theme_id: themeId }),
  });
  const json = await jsonOrThrow<{ data: { theme_id: string } }>(res);
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

async function showExtension<T>(slug: string, extId: string, id: string): Promise<T | null> {
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
