// visibility — 선언적 VisibilityRule 을 클라이언트가 평가.
// 코어는 규칙을 직렬화해 내려주고, 평가는 이 파일에서만 한다 (서버 구동 클로저 회피).
import type { ExtensionStepInfo, VisibilityRule } from "./api";

/// outcomes: stepId → (field → value). 각 step 의 StepOutcome.values 를 누적.
export function evalRule(
  rule: VisibilityRule,
  outcomes: Map<string, Map<string, string>>,
): boolean {
  const get = (sid: string, f: string) => outcomes.get(sid)?.get(f) ?? "";
  switch (rule.kind) {
    case "field_not_empty":
      return get(rule.step_id, rule.field).trim() !== "";
    case "field_equals":
      return get(rule.step_id, rule.field) === rule.value;
    case "all":
      return rule.all.every((r) => evalRule(r, outcomes));
    case "any":
      return rule.any.some((r) => evalRule(r, outcomes));
  }
}

/// outcome(values) 를 outcomes 맵에 머지한 새 맵 반환 (불변).
export function mergeOutcome(
  outcomes: Map<string, Map<string, string>>,
  stepId: string,
  values: Record<string, string>,
): Map<string, Map<string, string>> {
  const next = new Map(outcomes);
  const m = new Map<string, string>();
  for (const [k, v] of Object.entries(values)) m.set(k, v);
  next.set(stepId, m);
  return next;
}

/// step 의 prefill 매핑을 사이트 컨텍스트 값으로 해석.
/// 코어는 source 식별자(예: "site_name")만 내려주고, 여기서 값을 채운다.
export function resolvePrefill(
  step: ExtensionStepInfo,
  siteName: string,
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [field, source] of Object.entries(step.prefill ?? {})) {
    if (source === "site_name" && siteName) out[field] = siteName;
  }
  return out;
}
