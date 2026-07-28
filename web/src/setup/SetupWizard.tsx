// SetupWizard — status 응답의 extension_steps/external_api_keys로 step을 동적 조립.
// 코어가 profile/movies/books를 모른다 — 확장이 자기 SetupStep으로 선언한다.

import { useEffect, useMemo, useState } from "react";
import {
  fetchSetupStatus,
  submitSite,
  submitExtensions,
  submitExtensionStep,
  submitExternalKeys,
  submitTheme,
  submitComplete,
  type CompleteResult,
  type ExtensionStepInfo,
  type SetupStatus,
} from "./api";
import { SetupGuard } from "./SetupGuard";
import { StepSite } from "./StepSite";
import { StepExtensions } from "./StepExtensions";
import { GenericStep } from "./GenericStep";
import { ExternalKeysStep } from "./ExternalKeysStep";
import { StepTheme } from "./StepTheme";
import { StepDone } from "./StepDone";

type Step =
  | { type: "site"; id: string }
  | { type: "extensions"; id: string }
  | { type: "extension-step"; id: string; step: ExtensionStepInfo }
  | { type: "external-keys"; id: string }
  | { type: "theme"; id: string }
  | { type: "done"; id: string };

function buildSteps(status: SetupStatus | null): Step[] {
  if (!status) return [];
  const out: Step[] = [
    { type: "site", id: "site" },
    { type: "extensions", id: "extensions" },
  ];
  for (const step of status.extension_steps ?? []) {
    out.push({ type: "extension-step", id: step.id, step });
  }
  if ((status.external_api_keys ?? []).length > 0) {
    out.push({ type: "external-keys", id: "external-keys" });
  }
  out.push({ type: "theme", id: "theme" });
  out.push({ type: "done", id: "done" });
  return out;
}

/// Step의 prefill 필드를 사이트 컨텍스트 값으로 해석.
/// 확장이 자기 도메인에서 어떤 컨텍스트 출처가 의미 있는지 선언한 것만 지원.
function resolvePrefill(
  step: ExtensionStepInfo,
  siteName: string,
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [field, source] of Object.entries(step.prefill ?? {})) {
    if (source === "site_name" && siteName) {
      out[field] = siteName;
    }
  }
  return out;
}

function StepIndicator({ current, total }: { current: number; total: number }) {
  return (
    <div className="flex gap-2 justify-center mb-8">
      {Array.from({ length: total }, (_, i) => (
        <div
          key={i}
          className={`h-2 rounded-full transition-colors ${
            i === current
              ? "bg-primary w-8"
              : i < current
                ? "bg-primary/40 w-2"
                : "bg-line w-2"
          }`}
        />
      ))}
    </div>
  );
}

export function SetupWizard() {
  const [stepIdx, setStepIdx] = useState(0);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [completeResult, setCompleteResult] = useState<CompleteResult | null>(
    null,
  );
  // site 1단계에서 입력한 이름. 확장의 prefill hint가 site_name을 요구하면 주입.
  const [siteName, setSiteName] = useState<string>("");

  const steps = useMemo(() => buildSteps(status), [status]);
  const current = steps[stepIdx];

  useEffect(() => {
    fetchSetupStatus()
      .then((s) => setStatus(s))
      .catch(() => {
        // If 410, setup is already done — redirect
        window.location.href = "/";
      });
  }, []);

  const handleNext = async (submit: () => Promise<unknown>) => {
    setLoading(true);
    try {
      await submit();
      setStepIdx((i) => i + 1);
    } catch (err) {
      console.error(`setup step failed:`, err);
      alert(`저장 중 오류: ${err instanceof Error ? err.message : "알 수 없는 오류"}`);
    } finally {
      setLoading(false);
    }
  };

  if (!status || !current) {
    return (
      <div className="min-h-screen flex items-center justify-center p-4">
        <p className="text-subtle">불러오는 중…</p>
      </div>
    );
  }

  if (completeResult) {
    return <StepDone result={completeResult} />;
  }

  const renderStep = () => {
    switch (current.type) {
      case "site":
        return (
          <StepSite
            loading={loading}
            onNext={(data) => {
              setSiteName(data.name);
              return handleNext(() =>
                submitSite(data as { name: string; base_url?: string }),
              );
            }}
          />
        );
      case "extensions":
        return (
          <StepExtensions
            extensions={status.available_extensions ?? []}
            loading={loading}
            onBack={() => setStepIdx((i) => Math.max(0, i - 1))}
            onNext={(data) =>
              handleNext(async () => {
                await submitExtensions(data as { enabled: string[] });
                // 활성 세트 변경 반영: extension_steps/external_api_keys는 is_active에
                // 의존하므로 status를 다시 받아 steps를 재조립해야 한다.
                const fresh = await fetchSetupStatus();
                setStatus(fresh);
              })
            }
          />
        );
      case "extension-step": {
        // 확장이 자기 SetupStep.prefill로 선언한 hint만 적용. 코드는 어떤 값이 가능한지도 모른다.
        const initial = resolvePrefill(current.step, siteName);
        return (
          <GenericStep
            step={current.step}
            initialValues={initial}
            loading={loading}
            onBack={() => setStepIdx((i) => Math.max(0, i - 1))}
            onNext={(form) =>
              handleNext(() => submitExtensionStep(current.step.id, form))
            }
          />
        );
      }
      case "external-keys":
        return (
          <ExternalKeysStep
            keys={status.external_api_keys ?? []}
            loading={loading}
            onBack={() => setStepIdx((i) => Math.max(0, i - 1))}
            onNext={(values) => handleNext(() => submitExternalKeys(values))}
          />
        );
      case "theme":
        return (
          <StepTheme
            themes={status.available_themes ?? []}
            loading={loading}
            onBack={() => setStepIdx((i) => Math.max(0, i - 1))}
            onNext={(data) =>
              handleNext(async () => {
                await submitTheme(
                  data as { theme_id: string; lobby_mode?: string },
                );
                // theme 저장 성공 후 곧바로 setup 완료 (서버가 seed_sample_data를 호출).
                const r = await submitComplete();
                setCompleteResult(r);
              })
            }
          />
        );
      case "done":
        return <StepDone result={{ ok: true, message: "" }} />;
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-4">
      <div className="w-full max-w-2xl">
        <StepIndicator current={stepIdx} total={steps.length} />
        {renderStep()}
      </div>
    </div>
  );
}
