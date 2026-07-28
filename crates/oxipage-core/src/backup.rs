//! 백업 (doc/05 §5.4).
//!
//! SQLite 연속 백업은 운영 레벨에서 `litestream`(WAL 스트리밍)으로 담당하고,
//! 이 모듈은 코드 레벨 폴백인 **`VACUUM INTO` 포인트-인-타임 스냅샷**을 제공한다.
//! `VACUUM INTO`는 온라인(읽기 차단 없음)으로 일관된 DB 복사본을 만든다.
//!
//! 미디어(`/data/media`) 백업은 restic/rclone 등 외부 도구 영역(운영 매뉴얼)이고,
//! 이 모듈은 SQLite만 커버한다 — litestream과 동일한 범위.

use sqlx::SqlitePool;
use std::path::Path;

/// `VACUUM INTO`로 DB의 일관된 스냅샷을 `dest`에 생성한다.
///
/// `dest`는 바인드 파라미터로 전달되어 SQL 삽입 여지가 없다. SQLite는
/// `VACUUM INTO`를 트랜잭션 밖에서 실행해야 하므로 단일 statement로 실행한다.
pub async fn vacuum_into(pool: &SqlitePool, dest: &Path) -> anyhow::Result<()> {
    let dest_str = dest
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("backup destination path is not valid UTF-8"))?;
    sqlx::query("VACUUM INTO ?1")
        .bind(dest_str)
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("VACUUM INTO failed: {e}"))?;
    Ok(())
}

/// 에포크 초 기반 스냅샷 파일명. chrono 의존성을 피하려고 에포크를 쓴다.
/// 예: `oxipage-1735689600.db`.
pub fn snapshot_filename(epoch_secs: u64) -> String {
    format!("oxipage-{epoch_secs}.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_is_deterministic() {
        assert_eq!(snapshot_filename(1735689600), "oxipage-1735689600.db");
    }

    #[tokio::test]
    async fn vacuum_into_creates_file() {
        let dir = std::env::temp_dir().join(format!("oxipage-backup-test-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let src = dir.join("src.db");
        let dest = dir.join("snap.db");

        // VACUUM INTO는 파일 기반 DB에서 안정적으로 동작한다.
        let pool = crate::db::connect(&src).await.unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t (v) VALUES ('hello')")
            .execute(&pool)
            .await
            .unwrap();

        vacuum_into(&pool, &dest).await.unwrap();
        assert!(dest.exists(), "VACUUM INTO must create the snapshot file");
        let meta = tokio::fs::metadata(&dest).await.unwrap();
        assert!(meta.len() > 0);

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
