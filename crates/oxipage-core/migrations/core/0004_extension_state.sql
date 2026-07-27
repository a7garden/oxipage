-- doc/02 §2.13 런타임 활성 상태 — 단일 진실 소스.
-- 모든 컴파일된 확장의 라우트는 항상 마운트되며, 미들웨어가 이 테이블로 /api/v1/{ext}/** 를 게이트한다.
-- `enabled`: 런타임 토글 (disable → 라우트 404 + FTS 즉시 정리).
-- `purged`:    데이터 삭제 표식. 부팅 시 purged 확장은 마이그레이션을 스킵하고,
--              `enable` 재호출이 플래그를 클리어하며 마이그레이션을 재실행한다.
-- toml [extensions].enabled는 첫 부팅 시드로만 쓰이고 이후엔 무시된다.
CREATE TABLE IF NOT EXISTS extension_state (
    extension_id TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 1,
    purged INTEGER NOT NULL DEFAULT 0,
    disabled_at TEXT
);
