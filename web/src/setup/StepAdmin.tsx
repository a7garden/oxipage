// Step 2: Admin 비밀번호 (doc/13 §13.7.2)

import { useState } from "react";
import { Button } from "../shared/ui/button";
import { Input } from "../shared/ui/input";

interface Props {
  onNext: (data: { password: string }) => void;
  onBack: () => void;
  loading: boolean;
}

export function StepAdmin({ onNext, onBack, loading }: Props) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState("");

  const handleNext = () => {
    if (password.length < 8) {
      setError("비밀번호는 8자 이상이어야 합니다");
      return;
    }
    if (password !== confirm) {
      setError("비밀번호가 일치하지 않습니다");
      return;
    }
    setError("");
    onNext({ password });
  };

  return (
    <div>
      <h2 className="text-xl font-semibold mb-6 text-center">관리자 비밀번호</h2>

      <label className="block text-sm font-medium mb-2">비밀번호</label>
      <Input
        type="password"
        placeholder="8자 이상"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        autoFocus
      />

      <label className="block text-sm font-medium mt-4 mb-2">비밀번호 확인</label>
      <Input
        type="password"
        placeholder="다시 입력"
        value={confirm}
        onChange={(e) => setConfirm(e.target.value)}
      />

      {error && <p className="text-sm text-red-500 mt-2">{error}</p>}
      <p className="text-xs text-subtle mt-1">
        이 비밀번호는 관리 콘솔 로그인에 사용됩니다.
      </p>

      <div className="flex justify-between mt-8">
        <Button variant="secondary" onClick={onBack}>
          ← 이전
        </Button>
        <Button onClick={handleNext} disabled={loading}>
          {loading ? "저장 중..." : "다음 →"}
        </Button>
      </div>
    </div>
  );
}
