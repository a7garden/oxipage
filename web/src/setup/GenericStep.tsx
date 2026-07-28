// GenericStep — ExtensionStepInfo의 fields[]를 받아 동적으로 input을 렌더.
// 코어가 확장의 도메인 필드를 모른다 — 확장이 SetupField 목록으로 정의한다.

import { useState } from "react";
import { Button } from "../shared/ui/button";
import { Input } from "../shared/ui/input";
import type { ExtensionStepInfo } from "./api";

interface Props {
  step: ExtensionStepInfo;
  /// 필드 pre-fill 값. 예: profile step에 siteName을 display_name으로 주입.
  initialValues?: Record<string, string>;
  onNext: (form: Record<string, string>) => void;
  onBack: () => void;
  loading: boolean;
}

export function GenericStep({ step, initialValues, onNext, onBack, loading }: Props) {
  // 필드별 로컬 상태. 시작은 initialValues 또는 빈 문자열.
  const [values, setValues] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      step.fields.map((f) => [f.name, initialValues?.[f.name] ?? ""]),
    ),
  );
  const set = (name: string, v: string) =>
    setValues((prev) => ({ ...prev, [name]: v }));

  const requiredOk = step.fields
    .filter((f) => f.required)
    .every((f) => values[f.name]?.trim());

  const renderField = (f: ExtensionStepInfo["fields"][number]) => {
    const value = values[f.name] ?? "";
    if (f.type === "textarea") {
      return (
        <div key={f.name} className="mt-4">
          <label className="block text-sm font-medium mb-2">{f.label_ko}</label>
          <textarea
            value={value}
            onChange={(e) => set(f.name, e.target.value)}
            placeholder={f.placeholder_ko ?? ""}
            rows={5}
            className="w-full px-3 py-2 border border-line rounded-md text-sm bg-surface font-mono"
          />
        </div>
      );
    }
    const inputType = f.type === "url" ? "url" : "text";
    return (
      <div key={f.name} className="mt-4">
        <label className="block text-sm font-medium mb-2">
          {f.label_ko}
          {f.required && <span className="text-error ml-1">*</span>}
        </label>
        <Input
          type={inputType}
          value={value}
          onChange={(e) => set(f.name, e.target.value)}
          placeholder={f.placeholder_ko ?? ""}
        />
      </div>
    );
  };

  const allOptional = step.fields.every((f) => !f.required);

  return (
    <div>
      <h2 className="text-xl font-semibold mb-2 text-center">{step.title_ko}</h2>
      <p className="text-sm text-subtle text-center mb-6">{step.description_ko}</p>

      <div>{step.fields.map(renderField)}</div>

      <div className="flex justify-between mt-8">
        <Button variant="secondary" onClick={onBack}>
          ← 이전
        </Button>
        <div className="flex gap-2">
          {allOptional && (
            <Button variant="ghost" onClick={() => onNext({})} disabled={loading}>
              건너뛰기
            </Button>
          )}
          <Button onClick={() => onNext(values)} disabled={!requiredOk || loading}>
            {loading ? "저장 중..." : "다음 →"}
          </Button>
        </div>
      </div>
    </div>
  );
}
