const CONSOLE_BASE = "/api/console";

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
    throw new Error(body?.error ?? `오류 (${res.status})`);
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

export async function siteScopedFetch(slug: string, path: string): Promise<Response> {
  return fetch(`${CONSOLE_BASE}/s/${slug}${path}`);
}

export async function triggerBuild(slug: string): Promise<Response> {
  return fetch(`${CONSOLE_BASE}/s/${slug}/build`, { method: "POST" });
}

export async function triggerDeploy(slug: string): Promise<Response> {
  return fetch(`${CONSOLE_BASE}/s/${slug}/deploy`, { method: "POST" });
}
