// ExternalKeysStep — 활성 확장이 노출한 ExternalApiKey[]를 받아 동적으로 키 입력란 렌더.

import { useState } from "react";
import { Button } from "../shared/ui/button";
import { Input } from "../shared/ui/input";
import type { ExternalApiKey } from "./api";

interface Props {
  keys: ExternalApiKey[];
  onNext: (values: Record<string, string>) => void;
  onBack: () => void;
  loading: boolean;
}

export function ExternalKeysStep({ keys, onNext, onBack, loading }: Props) {
  const [values, setValues] = useState<Record<string, string>>(() =>
    Object.fromEntries(keys.map((k) => [k.id, ""])),
  );

  const set = (id: string, v: string) =>
    setValues((prev) => ({ ...prev, [id]: v }));

  return (
    <div>
      <h2 className="text-xl font-semibold mb-2 text-center">외부 API 키</h2>
      <p className="text-sm text-subtle text-center mb-6">
        모두 선택 사항입니다. 나중에 관리 콘솔에서도 변경할 수 있습니다.
      </p>

      <div className="space-y-4">
        {keys.map((k) => (
          <div key={k.id}>
            <label className="block text-sm font-medium mb-1">
              {k.label_ko}
              {!k.required && (
                <span className="text-subtle ml-2 text-xs">(선택)</span>
              )}
            </label>
            <Input
              type="password"
              value={values[k.id] ?? ""}
              onChange={(e) => set(k.id, e.target.value)}
              placeholder={`${k.env_var} 환경변수`}
            />
            <p className="text-xs text-subtle mt-1">
              {k.env_var}에 저장됩니다
            </p>
          </div>
        ))}
      </div>

      <div className="flex justify-between mt-8">
        <Button variant="secondary" onClick={onBack}>
          ← 이전
        </Button>
        <div className="flex gap-2">
          <Button variant="ghost" onClick={() => onNext({})} disabled={loading}>
            건너뛰기
          </Button>
          <Button onClick={() => onNext(values)} disabled={loading}>
            {loading ? "저장 중..." : "다음 →"}
          </Button>
        </div>
      </div>
    </div>
  );
}
