//! 확장 레지스트리 (doc/01 §1.4, doc/02 §2.13).
//!
//! **단일 진실 소스 모델.** 모든 컴파일된 확장이 레지스트리에 들어가 라우트까지 항상
//! 마운트된다. `oxipage.toml`의 `[extensions].enabled`는 첫 부팅 시드로만 쓰이고, 이후
//! 런타임 상태는 DB `extension_state` 테이블이 유일하게 결정한다. 이래야 `enable`/`disable`
//! 이 대칭적·즉시 효과를 갖는다 (라우트는 이미 마운트되어 있고 미들웨어가 게이트).
//!
//! - `enabled=false`: soft disable — 라우트 404 + `on_disable()`로 FTS 즉시 정리. DB/미디어 유지.
//! - `purged=true`:   hard purge 흔적 — 부팅 시 마이그레이션 스킵, `enable`이 플래그 클리어 +
//!   마이그레이션 재실행으로 복구. 데이터는 이미 DROP되었으므로 빈 테이블로 재생성.

use crate::extension::Extension;
use crate::migrate;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeStatus {
    pub enabled: bool,
    pub purged: bool,
}

impl RuntimeStatus {
    /// 미들웨어/매니페스트가 쓰는 "라우트가 살아있는가" = enabled && !purged.
    pub fn active(self) -> bool {
        self.enabled && !self.purged
    }
}

pub struct ExtensionRegistry {
    extensions: Vec<Arc<dyn Extension>>,
    runtime: RwLock<HashMap<String, RuntimeStatus>>,
}

impl ExtensionRegistry {
    pub fn new(extensions: Vec<Arc<dyn Extension>>) -> Self {
        Self {
            extensions,
            runtime: RwLock::new(HashMap::new()),
        }
    }

    pub fn find(&self, id: &str) -> Option<&Arc<dyn Extension>> {
        self.extensions.iter().find(|e| e.id() == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Extension>> {
        self.extensions.iter()
    }

    /// 부팅 시 DB에서 런타임 상태 로드. 행이 없으면 toml enabled 목록으로 시드:
    /// toml이 비어있거나 id를 포함 → enabled=1, 아니면 enabled=0. purged=0 기본.
    /// 이미 purge된 확장은 그 상태를 존중한다.
    pub async fn seed_runtime_state(
        &self,
        pool: &SqlitePool,
        toml_enabled: &[String],
    ) -> anyhow::Result<()> {
        // DB I/O를 write lock 바깥에서 수행 (lock을 디스크 I/O 동안 hold하면
        // extension_state 접근이 직렬화되고 데드락 위험이 있다).
        let mut entries = Vec::new();
        for ext in &self.extensions {
            let row: Option<(i64, i64)> =
                sqlx::query_as("SELECT enabled, purged FROM extension_state WHERE extension_id = ?")
                    .bind(ext.id())
                    .fetch_optional(pool)
                    .await?;
            let status = match row {
                Some((e, p)) => RuntimeStatus {
                    enabled: e != 0,
                    purged: p != 0,
                },
                None => {
                    let enabled = toml_enabled.is_empty()
                        || toml_enabled.iter().any(|id| id == ext.id());
                    sqlx::query(
                        "INSERT INTO extension_state (extension_id, enabled, purged)
                         VALUES (?1, ?2, 0)",
                    )
                    .bind(ext.id())
                    .bind(if enabled { 1i64 } else { 0 })
                    .execute(pool)
                    .await?;
                    RuntimeStatus {
                        enabled,
                        purged: false,
                    }
                }
            };
            entries.push((ext.id().to_string(), status));
        }
        // cache 갱신 — lock은 이 짧은 구간만.
        let mut cache = self.runtime.write().await;
        cache.clear();
        for (id, status) in entries {
            cache.insert(id, status);
        }
        Ok(())
    }

    pub async fn status_of(&self, id: &str) -> Option<RuntimeStatus> {
        self.runtime.read().await.get(id).copied()
    }

    /// 라우트가 살아있는가 (enabled && !purged). 미들웨어가 사용.
    pub async fn is_active(&self, id: &str) -> bool {
        self.status_of(id).await.map(|s| s.active()).unwrap_or(false)
    }

    pub async fn status_snapshot(&self) -> HashMap<String, RuntimeStatus> {
        self.runtime.read().await.clone()
    }

    /// enabled 토글. 이전 상태 반환. purged 플래그는 건드리지 않는다.
    pub async fn set_enabled(
        &self,
        pool: &SqlitePool,
        id: &str,
        enabled: bool,
    ) -> anyhow::Result<Option<RuntimeStatus>> {
        let prev = self.status_of(id).await;
        sqlx::query(
            "INSERT INTO extension_state (extension_id, enabled, purged, disabled_at)
             VALUES (?1, ?2, 0,
                     CASE WHEN ?2 = 0 THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE NULL END)
             ON CONFLICT(extension_id) DO UPDATE SET
                enabled = ?2,
                disabled_at = CASE WHEN ?2 = 0
                    THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE NULL END",
        )
        .bind(id)
        .bind(if enabled { 1i64 } else { 0 })
        .execute(pool)
        .await?;
        if let Some(s) = self.runtime.write().await.get_mut(id) {
            s.enabled = enabled;
        }
        Ok(prev)
    }

    /// purge 플래그 토글. true 면 enabled도 0으로 (데이터가 사라졌으므로).
    /// enable-after-purge 경로는 purged=false 로 여기를 부른 뒤 run_migrations.
    pub async fn set_purged(
        &self,
        pool: &SqlitePool,
        id: &str,
        purged: bool,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO extension_state (extension_id, enabled, purged)
             VALUES (?1, 0, ?2)
             ON CONFLICT(extension_id) DO UPDATE SET purged = ?2, enabled = CASE WHEN ?2 = 1 THEN 0 ELSE enabled END",
        )
        .bind(id)
        .bind(if purged { 1i64 } else { 0 })
        .execute(pool)
        .await?;
        if let Some(s) = self.runtime.write().await.get_mut(id) {
            s.purged = purged;
            if purged {
                s.enabled = false;
            }
        }
        Ok(())
    }

    /// 코어 마이그레이션 → 런타임 상태 시드 → 확장 마이그레이션(purged가 아닌 것만).
    /// seed가 코어 마이그레이션(0004 extension_state 테이블 생성) 뒤에 와야 하고,
    /// 확장 마이그레이션의 purged-skip 체크가 cache를 읽으려면 seed가 먼저여야 한다.
    pub async fn run_migrations(
        &self,
        pool: &SqlitePool,
        toml_enabled: &[String],
    ) -> anyhow::Result<()> {
        migrate::run_migrations(pool, "_core", migrate::CORE_MIGRATIONS).await?;
        self.seed_runtime_state(pool, toml_enabled).await?;
        for ext in &self.extensions {
            let purged = self
                .status_of(ext.id())
                .await
                .map(|s| s.purged)
                .unwrap_or(false);
            if !purged {
                migrate::run_migrations(pool, ext.id(), &ext.migrations()).await?;
            }
        }
        Ok(())
    }

    /// 특정 확장의 마이그레이션을 강제 재실행 (enable-after-purge). schema_migrations 행을
    /// 먼저 비우면 run_migrations가 버전 무관하게 재적용한다.
    pub async fn rerun_migrations(&self, pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
        let Some(migrations) = self.find(id).map(|e| e.migrations()) else {
            return Ok(());
        };
        sqlx::query("DELETE FROM schema_migrations WHERE extension = ?")
            .bind(id)
            .execute(pool)
            .await?;
        migrate::run_migrations(pool, id, &migrations).await?;
        Ok(())
    }
}
