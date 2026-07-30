// 완료 화면 — 사이트 대시보드로 자동 진입 (spec D5).
// wizard의 마지막 단계에서 호출되면 setup_state.setup_completed_at을 마킹하고
// 사용자를 /s/{slug}/ 로 보낸다 (관리 콘솔 === 메인 서피스).

import { Check } from "lucide-react";
import { useEffect } from "react";
import { Button } from "../shared/ui/button";
import type { CompleteResult } from "./api";

interface Props {
  result: CompleteResult;
  slug: string;
}

export function StepDone({ result, slug }: Props) {
  useEffect(() => {
    // 자동으로 사이트 대시보드로 이동.
    // 사용자가 버튼을 클릭하지 않아도 1초 뒤 redirect.
    const t = window.setTimeout(() => {
      window.location.href = `/s/${slug}/`;
    }, 1200);
    return () => window.clearTimeout(t);
  }, [slug]);

  return (
    <div className="text-center">
      <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-primary/10 text-primary">
        <Check className="size-6" strokeWidth={2.5} />
      </div>
      <h2 className="text-xl font-semibold mb-2">설정 완료!</h2>
      <p className="text-sm text-subtle mb-6">{result.message}</p>

      <Button onClick={() => (window.location.href = `/s/${slug}/`)}>
        콘솔 열기
      </Button>
    </div>
  );
}
