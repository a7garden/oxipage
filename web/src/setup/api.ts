// Setup wizard API client (doc/13 §13.7.3, 2026-07-29 native-treatment redesign)

const BASE = "/api/console/setup";

export interface SetupStatus {
  setup_mode: boolean;
  completed_steps?: string[];
  available_extensions?: ExtensionInfo[];
  available_themes?: ThemeInfo[];
  /// 활성 확장이 소유한 서브-위자드 (동적 조립).
  extension_wizards?: ExtensionWizardInfo[];
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
/// 확장이 자기 SetupStep으로 자기 도메인 데이터 + prefill hint를 선언.
export interface ExtensionStepInfo {
  id: string;
  title_ko: string;
  title_en: string;
  description_ko: string;
  description_en: string;
  fields: SetupField[];
  /// 필드 pre-fill 매핑. wizard가 사이트 컨텍스트에서 값을 가져와 채운다.
  /// 예: `{"display_name": "site_name"}` → site_name을 display_name에 주입.
  /// 코어는 키 이름도, 가능한 source 값도 모른다 — 확장이 자기 SetupStep으로 선언.
  prefill?: Record<string, string>;
  /// step 표시 조건 (클라이언트 평가). 없으면 항상 표시 (Phase 3).
  visible_when?: VisibilityRule;
  /// fields 가 비어있으면 action step (버튼만).
  is_action?: boolean;
}

/// step 가시성 규칙. 코어가 직렬화해 내려주면 클라이언트가 evalRule 로 평가 (Phase 3).
export type VisibilityRule =
  | { kind: "field_not_empty"; step_id: string; field: string }
  | { kind: "field_equals"; step_id: string; field: string; value: string }
  | { kind: "all"; all: VisibilityRule[] }
  | { kind: "any"; any: VisibilityRule[] };

/// 한 확장의 서브-위자드 (status 응답). Phase 3에서 ExtensionSubWizard가 소비.
export interface ExtensionWizardInfo {
  extension_id: string;
  display_name: { ko: string; en: string };
  steps: ExtensionStepInfo[];
}

export interface SetupField {
  name: string;
  label_ko: string;
  label_en: string;
  type: "text" | "textarea" | "url" | "secret";
  required: boolean;
  placeholder_ko?: string | null;
  placeholder_en?: string | null;
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

/// Setup step 1: create-or-register an oxipage project directory.
/// Returns the slug and registered path so later steps can target it.
export async function createSite(path: string): Promise<{ data: { slug: string; path: string } }> {
  return post<{ data: { slug: string; path: string } }>("/create-site", { path });
}

export async function submitExtensions(data: { enabled: string[] }) {
  return post<{ enabled: string[] }>("/extensions", data);
}

/// 특정 확장 step의 form을 저장. extId/stepId는 status 응답의 extension_wizards에 있다.
export async function submitExtensionStep(
  extId: string,
  stepId: string,
  form: Record<string, string>,
) {
  return post<{ values: Record<string, string> }>(
    `/extension-step/${encodeURIComponent(extId)}/${encodeURIComponent(stepId)}`,
    form,
  );
}


export async function submitTheme(data: { theme_id: string; lobby_mode?: string }) {
  return post<{ ok: boolean }>("/theme", data);
}

export async function submitComplete(): Promise<CompleteResult> {
  return post<CompleteResult>("/complete", {});
}
