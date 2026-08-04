import { useState } from "react";
import { useNavigate } from "react-router";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createSite } from "../shared/api";
import { PageHeader, PageTitle } from "../../shared/ui/page-header";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Label } from "../../shared/ui/label";
import { Card, CardContent, CardDescription } from "../../shared/ui/card";

export function NewSiteWizardPage() {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [path, setPath] = useState("");
  const [error, setError] = useState<string | null>(null);

  const create = useMutation({
    mutationFn: () => createSite(path.trim()),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["sites"] });
      navigate("/sites");
    },
    onError: (err) => {
      setError(err instanceof Error ? err.message : "알 수 없는 오류");
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!path.trim()) { setError("사이트 경로를 입력하세요."); return; }
    create.mutate();
  };

  return (
    <div className="max-w-lg mx-auto">
      <PageHeader>
        <PageTitle>새 사이트 추가</PageTitle>
      </PageHeader>

      <Card>
        <CardContent className="pt-6">
          <form onSubmit={handleSubmit} className="space-y-5">
            <div className="space-y-2">
              <Label htmlFor="site-path">사이트 경로</Label>
              <Input
                id="site-path"
                placeholder="~/oxibuilder/blog"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                disabled={create.isPending}
                autoFocus
              />
              <p className="text-xs text-muted">
                oxibuilder.toml이 없는 경로는 자동으로 생성됩니다. 사이트 이름(slug)은
                디렉토리 이름이 됩니다.
              </p>
            </div>

            <CardDescription className="text-xs">
              예: "~/oxibuilder/blog" 입력 시 slug는 "blog"가 됩니다.
            </CardDescription>

            {error && (
              <p className="text-sm text-destructive">{error}</p>
            )}

            <div className="flex items-center gap-3 pt-2">
              <Button type="submit" disabled={create.isPending}>
                {create.isPending ? "생성 중..." : "생성"}
              </Button>
              <Button
                type="button"
                variant="ghost"
                onClick={() => navigate("/sites")}
                disabled={create.isPending}
              >
                취소
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
