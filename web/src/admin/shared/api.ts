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

export async function createSite(name: string, path: string, default_?: boolean): Promise<Response> {
  return fetch(`${CONSOLE_BASE}/sites`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ name, path, default: default_ }),
  });
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
