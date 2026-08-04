// Step 1: 사이트 디렉토리 결정 (v2 SSG site-picker, spec D5).
//
// 위저드 시작 = 사이트 시작. 사용자가 신규 디렉토리를 만들거나
// 기존 oxibuilder 디렉토리를 등록한다. Step 완료 시 dashboard로 진입한다.

import { useState } from "react";
import { Button } from "../shared/ui/button";
import { Input } from "../shared/ui/input";
import { createSite } from "./api";

interface Props {
  onNext: (data: { slug: string; path: string }) => void;
  loading: boolean;
  setLoading: (loading: boolean) => void;
}

export function StepSite({ onNext, loading, setLoading }: Props) {
  const [sitePath, setSitePath] = useState("~/oxibuilder/blog");
  const [error, setError] = useState<string | null>(null);
  const valid = sitePath.trim().length > 0;

  async function submit() {
    if (!valid) return;
    setError(null);
    setLoading(true);
    try {
      const { data } = await createSite(sitePath.trim());
      onNext(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : "create failed");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div>
      <h2 className="text-xl font-semibold mb-6 text-center">사이트 디렉토리</h2>
      <p className="text-sm text-subtle text-center mb-6">
        새 디렉토리를 만들거나 기존 oxibuilder 디렉토리를 등록합니다.
      </p>

      <label className="block text-sm font-medium mb-2">경로</label>
      <Input
        placeholder="~/oxibuilder/blog"
        value={sitePath}
        onChange={(e) => setSitePath(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") submit();
        }}
        autoFocus
      />
      <p className="text-xs text-subtle mt-1">
        oxibuilder.toml이 없는 경로는 자동 생성됩니다.
      </p>

      {error && (
        <p className="text-sm text-error mt-3" role="alert">
          {error}
        </p>
      )}

      <div className="flex justify-end mt-8">
        <Button onClick={submit} disabled={!valid || loading}>
          {loading ? "등록 중..." : "다음 →"}
        </Button>
      </div>
    </div>
  );
}
