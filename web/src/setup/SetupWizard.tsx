// SetupWizard — 6-step 설정 마법사 (doc/13 §13.7.3)

import { useEffect, useState } from "react";
import {
  fetchSetupStatus,
  submitSite,
  submitAdmin,
  submitExtensions,
  submitProfile,
  submitTheme,
  submitContent,
  submitComplete,
  type CompleteResult,
  type SetupStatus,
} from "./api";
import { SetupGuard } from "./SetupGuard";
import { StepSite } from "./StepSite";
import { StepAdmin } from "./StepAdmin";
import { StepExtensions } from "./StepExtensions";
import { StepProfile } from "./StepProfile";
import { StepTheme } from "./StepTheme";
import { StepContent } from "./StepContent";
import { StepDone } from "./StepDone";

const STEP_NAMES = ["site", "admin", "extensions", "profile", "theme", "content"];

function StepIndicator({ current, total }: { current: number; total: number }) {
  return (
    <div className="flex gap-2 justify-center mb-8">
      {Array.from({ length: total }, (_, i) => (
        <div
          key={i}
          className={`h-2 rounded-full transition-all duration-300 ${
            i <= current ? "w-8 bg-[var(--color-accent)]" : "w-2 bg-[var(--color-border)]"
          }`}
        />
      ))}
    </div>
  );
}

export function SetupWizard() {
  const [step, setStep] = useState(0);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [completeResult, setCompleteResult] = useState<CompleteResult | null>(null);
  const [siteName, setSiteName] = useState("");

  useEffect(() => {
    fetchSetupStatus()
      .then((s) => {
        setStatus(s);
        // Resume from last completed step
        const completed = s.completed_steps ?? [];
        const lastIdx = Math.max(
          0,
          ...STEP_NAMES.map((name, i) => (completed.includes(name) ? i + 1 : -1)),
        );
        if (lastIdx > 0 && lastIdx <= STEP_NAMES.length) {
          setStep(lastIdx);
        }
      })
      .catch(() => {
        // If 410, setup is already done — redirect
        window.location.href = "/";
      });
  }, []);

  const handleNext = async (stepName: string, data: unknown, saveFn: (d: unknown) => Promise<unknown>) => {
    setLoading(true);
    try {
      await saveFn(data);
      const nextIdx = STEP_NAMES.indexOf(stepName) + 1;
      if (nextIdx < STEP_NAMES.length) {
        setStep(nextIdx);
      }
    } catch (err) {
      console.error(`setup step ${stepName} failed:`, err);
      alert(`저장 중 오류: ${err instanceof Error ? err.message : "알 수 없는 오류"}`);
    } finally {
      setLoading(false);
    }
  };

  const handleFinish = async () => {
    setLoading(true);
    try {
      const result = await submitComplete();
      setCompleteResult(result);
      setStep(STEP_NAMES.length + 1);
    } catch (err) {
      console.error("setup complete failed:", err);
      alert(`완료 처리 중 오류: ${err instanceof Error ? err.message : "알 수 없는 오류"}`);
    } finally {
      setLoading(false);
    }
  };

  if (!status) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="animate-pulse text-subtle">Loading...</div>
      </div>
    );
  }

  // Done screen
  if (completeResult) {
    return (
      <div className="min-h-screen flex items-center justify-center p-4">
        <div className="w-full max-w-lg">
          <StepIndicator current={STEP_NAMES.length + 1} total={STEP_NAMES.length + 1} />
          <StepDone result={completeResult} />
        </div>
      </div>
    );
  }

  const renderStep = () => {
    switch (step) {
      case 0:
        return (
          <StepSite
            onNext={(data) => {
              setSiteName(data.name);
              handleNext("site", data, (d) => submitSite(d as { name: string; base_url?: string }));
            }}
            loading={loading}
          />
        );
      case 1:
        return (
          <StepAdmin
            onNext={(data) => handleNext("admin", data, (d) => submitAdmin(d as { password: string }))}
            onBack={() => setStep(0)}
            loading={loading}
          />
        );
      case 2:
        return (
          <StepExtensions
            extensions={status.available_extensions ?? []}
            onNext={(data) =>
              handleNext("extensions", data, (d) => submitExtensions(d as { enabled: string[] }))
            }
            onBack={() => setStep(1)}
            loading={loading}
          />
        );
      case 3:
        return (
          <StepProfile
            siteName={siteName}
            onNext={(data) => handleNext("profile", data, (d) => submitProfile(d as Parameters<typeof submitProfile>[0]))}
            onBack={() => setStep(2)}
            loading={loading}
          />
        );
      case 4:
        return (
          <StepTheme
            themes={status.available_themes ?? []}
            onNext={(data) =>
              handleNext("theme", data, (d) => submitTheme(d as { theme_id: string; lobby_mode?: string }))
            }
            onBack={() => setStep(3)}
            loading={loading}
          />
        );
      case 5:
        return (
          <StepContent
            onNext={(data) =>
              handleNext("content", data, (d) => submitContent(d as Parameters<typeof submitContent>[0]))
            }
            onBack={() => setStep(4)}
            loading={loading}
            onFinish={handleFinish}
          />
        );
      default:
        return null;
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-4">
      <div className="w-full max-w-lg">
        <StepIndicator current={step} total={STEP_NAMES.length + 1} />
        {renderStep()}
      </div>
    </div>
  );
}
