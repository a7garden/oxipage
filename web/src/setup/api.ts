// Setup wizard API client (doc/13 §13.7.3)

const BASE = "/api/v1/setup";

export interface SetupStatus {
  setup_mode: boolean;
  completed_steps?: string[];
  available_extensions?: ExtensionInfo[];
  available_themes?: ThemeInfo[];
}

export interface ExtensionInfo {
  id: string;
  display_name: { ko: string; en: string };
}

export interface ThemeInfo {
  id: string;
  name_ko: string;
  name_en: string;
  mode: string;
  preview_colors: string[];
}

export interface CompleteResult {
  ok: boolean;
  token: string;
  token_label: string;
  message: string;
}

async function post<T>(path: string, data: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  });
  if (res.status === 410) throw new SetupCompletedError();
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body?.error?.message || `setup ${path}: ${res.status}`);
  }
  return res.json().then((r) => r.data as T);
}

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`);
  if (res.status === 410) throw new SetupCompletedError();
  if (!res.ok) throw new Error(`setup ${path}: ${res.status}`);
  return res.json().then((r) => r.data as T);
}

export class SetupCompletedError extends Error {
  constructor() {
    super("setup already completed");
    this.name = "SetupCompletedError";
  }
}

export async function fetchSetupStatus(): Promise<SetupStatus> {
  return get<SetupStatus>("/status");
}

export async function submitSite(data: { name: string; base_url?: string }) {
  return post<{ ok: boolean }>("/site", data);
}

export async function submitAdmin(data: { password: string }) {
  return post<{ ok: boolean }>("/admin", data);
}

export async function submitExtensions(data: { enabled: string[] }) {
  return post<{ enabled: string[] }>("/extensions", data);
}

export async function submitProfile(data: {
  display_name?: string;
  tagline_ko?: string;
  tagline_en?: string;
  github_username?: string;
  bio_ko?: string;
  bio_en?: string;
}) {
  return post<{ ok: boolean }>("/profile", data);
}

export async function submitTheme(data: { theme_id: string; lobby_mode?: string }) {
  return post<{ ok: boolean }>("/theme", data);
}

export async function submitContent(data: {
  sample_post?: boolean;
  tmdb_key?: string;
  aladin_key?: string;
}) {
  return post<{ ok: boolean }>("/content", data);
}

export async function submitComplete(): Promise<CompleteResult> {
  return post<CompleteResult>("/complete", {});
}
