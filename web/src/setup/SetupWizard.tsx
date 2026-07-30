// SetupWizard — status 응답의 extension_wizards로 step을 동적 조립.
// 각 활성 확장은 자기 서브-위자드(ExtensionSubWizard)를 전역 시퀀스의 한 칸으로 소유한다.

import { useEffect, useMemo, useState } from "react";
import {
  fetchSetupStatus,
  submitSite,
  submitExtensions,
  submitTheme,
  submitComplete,
  type CompleteResult,
  type ExtensionWizardInfo,
  type SetupStatus,
} from "./api";
import { SetupGuard } from "./SetupGuard";
import { StepSite } from "./StepSite";
import { StepExtensions } from "./StepExtensions";
import { StepTheme } from "./StepTheme";
import { StepDone } from "./StepDone";
import { ExtensionSubWizard } from "./ExtensionSubWizard";

type Step =
  | { type: "site" }
  | { type: "extensions" }
  | { type: "extension-wizard"; wizard: ExtensionWizardInfo }
  | { type: "theme" }
  | { type: "done" };

function buildSteps(status: SetupStatus | null): Step[] {
  if (!status) return [];
  const out: Step[] = [{ type: "site" }, { type: "extensions" }];
  for (const wizard of status.extension_wizards ?? []) {
    out.push({ type: "extension-wizard", wizard });
  }
  out.push({ type: "theme" });
  out.push({ type: "done" });
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
        // 410 이면 setup 이미 완료 — 홈으로.
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
    return <StepDone result={completeResult} slug={siteName} />;
  }

  const renderStep = () => {
    switch (current.type) {
      case "site":
        return (
          <StepSite
            loading={loading}
            setLoading={setLoading}
            onNext={(data) => {
              setSiteName(data.slug);
              return handleNext(async () => {
                await submitSite({ name: data.slug });
              });
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
                // 활성 세트 변경 반영: extension_wizards는 is_active에 의존.
                const fresh = await fetchSetupStatus();
                setStatus(fresh);
              })
            }
          />
        );
      case "extension-wizard":
        return (
          <ExtensionSubWizard
            wizard={current.wizard}
            siteName={siteName}
            onComplete={() => setStepIdx((i) => i + 1)}
            onExitBack={() => setStepIdx((i) => Math.max(0, i - 1))}
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
                // theme 저장 성공 후 곧바로 setup 완료 (서버가 seed_sample_data 호출).
                const r = await submitComplete();
                setCompleteResult(r);
              })
            }
          />
        );
      case "done":
        return <StepDone result={{ ok: true, message: "" }} slug={siteName} />;
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
