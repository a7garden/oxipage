// 완료 화면 (doc/13 §13.7.2)

import { Check } from "lucide-react";
import { Button } from "../shared/ui/button";
import type { CompleteResult } from "./api";

interface Props {
  result: CompleteResult;
}

export function StepDone({ result }: Props) {
  return (
    <div className="text-center">
      <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-primary/10 text-primary">
        <Check className="size-6" strokeWidth={2.5} />
      </div>
      <h2 className="text-xl font-semibold mb-2">설정 완료!</h2>
      <p className="text-sm text-subtle mb-6">{result.message}</p>

      <div className="flex gap-3 justify-center">
        <Button onClick={() => (window.location.href = "/")}>
          사이트 보기
        </Button>
        <Button
          variant="secondary"
          onClick={() => window.open("http://127.0.0.1:8788", "_blank")}
        >
          관리 콘솔
        </Button>
      </div>
    </div>
  );
}
