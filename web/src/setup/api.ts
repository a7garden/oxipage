// Setup wizard API client (doc/13 §13.7.3, 2026-07-29 native-treatment redesign)

const BASE = "/api/console/setup";

export interface SetupStatus {
  setup_mode: boolean;
  completed_steps?: string[];
  available_extensions?: ExtensionInfo[];
  available_themes?: ThemeInfo[];
  /// 활성 확장이 노출한 setup step (동적 조립).
  extension_steps?: ExtensionStepInfo[];
  /// 활성 확장이 노출한 외부 API 키 (동적 조립).
  external_api_keys?: ExternalApiKey[];
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

/// 동적 step 정보. web에서 setup wizard가 step을 그리는데 사용.
export interface ExtensionStepInfo {
  id: string;
  title_ko: string;
  title_en: string;
  description_ko: string;
  description_en: string;
  fields: SetupField[];
}

export interface SetupField {
  name: string;
  label_ko: string;
  label_en: string;
  type: "text" | "textarea" | "url";
  required: boolean;
  placeholder_ko?: string | null;
  placeholder_en?: string | null;
}

/// 외부 API 키. 마법사가 동적으로 키 입력란을 만들고, save 시 id로 dispatch.
export interface ExternalApiKey {
  id: string;
  label_ko: string;
  label_en: string;
  env_var: string;
  required: boolean;
  scope: "env_only" | "extension_config";
}

export interface CompleteResult {
  ok: boolean;
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

export async function submitExtensions(data: { enabled: string[] }) {
  return post<{ enabled: string[] }>("/extensions", data);
}

/// 특정 확장 step의 form을 저장. step.id는 status 응답의 extension_steps에 있다.
export async function submitExtensionStep(
  stepId: string,
  form: Record<string, string>,
) {
  return post<{ ok: boolean }>(`/extension-step/${encodeURIComponent(stepId)}`, form);
}

/// 활성 확장이 노출한 모든 외부 API 키를 한 번에 저장.
export async function submitExternalKeys(values: Record<string, string>) {
  return post<{ ok: boolean }>("/external-keys", { values });
}

export async function submitTheme(data: { theme_id: string; lobby_mode?: string }) {
  return post<{ ok: boolean }>("/theme", data);
}

export async function submitComplete(): Promise<CompleteResult> {
  return post<CompleteResult>("/complete", {});
}
