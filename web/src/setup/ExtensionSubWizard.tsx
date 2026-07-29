// ExtensionSubWizard — 한 확장의 서브-위자드를 소유.
// 내부 step 네비게이션 + outcomes 누적 + evalRule 가시성 평가.
// 코어는 모든 step + visible_when 규칙을 내려주고, 이 컴포넌트가 표시할 step을 고른다.

import { useEffect, useMemo, useState } from "react";
import type { ExtensionWizardInfo } from "./api";
import { submitExtensionStep } from "./api";
import { evalRule, mergeOutcome, resolvePrefill } from "./visibility";
import { GenericStep } from "./GenericStep";

interface Props {
  wizard: ExtensionWizardInfo;
  /// site 1단계 이름. step 이 prefill 로 site_name 을 요구하면 주입.
  siteName: string;
  onComplete: () => void;
  onExitBack: () => void;
}

export function ExtensionSubWizard({
  wizard,
  siteName,
  onComplete,
  onExitBack,
}: Props) {
  const [stepIdx, setStepIdx] = useState(0);
  const [outcomes, setOutcomes] = useState<Map<string, Map<string, string>>>(
    () => new Map(),
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const visibleSteps = useMemo(
    () =>
      wizard.steps.filter(
        (s) => !s.visible_when || evalRule(s.visible_when, outcomes),
      ),
    [wizard.steps, outcomes],
  );
  const current = visibleSteps[stepIdx];
  const done = stepIdx >= visibleSteps.length;

  // 모든 visible step 이 끝나면 부모에게 완료 알림.
  useEffect(() => {
    if (done) onComplete();
  }, [done, onComplete]);

  if (done) return null;

  const isFirst = stepIdx === 0;
  const onBack = isFirst
    ? onExitBack
    : () => setStepIdx((i) => Math.max(0, i - 1));
  const initial = resolvePrefill(current, siteName);

  const handleNext = async (form: Record<string, string>) => {
    setLoading(true);
    setError(null);
    try {
      const outcome = await submitExtensionStep(
        wizard.extension_id,
        current.id,
        form,
      );
      setOutcomes((prev) => mergeOutcome(prev, current.id, outcome.values ?? {}));
      setStepIdx((i) => i + 1);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <div className="text-center mb-1 text-xs text-subtle">
        {wizard.display_name.ko} · {stepIdx + 1}/{visibleSteps.length}
      </div>
      <GenericStep
        step={current}
        initialValues={initial}
        loading={loading}
        onBack={onBack}
        onNext={handleNext}
      />
      {error && <p className="text-error text-sm text-center mt-4">{error}</p>}
    </div>
  );
}
