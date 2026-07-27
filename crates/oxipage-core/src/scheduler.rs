//! 백그라운드 잡 스케줄러 (doc/01 §1.9).
//!
//! 코어가 단일 `tokio-cron-scheduler` 인스턴스를 띄우고, 각 확장이
//! `Extension::background_jobs()`로 등록한 잡을 실행한다.
//!
//! Phase 2에서 movies/books/scraps/activity 확장이 잡을 등록한다.

use async_trait::async_trait;
use std::sync::Arc;

/// 스케줄된 잡. cron 식과 실행 함수를 캡슐화.
#[async_trait]
pub trait ScheduledJob: Send + Sync {
    /// cron 식 (예: "0 */15 * * * *" = 15분마다).
    fn schedule(&self) -> &str;

    /// 잡 식별자 (로깅/모니터링용).
    fn name(&self) -> &str;

    /// 잡 실행. 실패해도 스케줄러가 다음 주기에 재시도한다 (에러 로깅만).
    async fn run(&self) -> anyhow::Result<()>;
}

/// 스케줄러는 단순한 in-process 크론. Phase 1에서는 잡이 없으므로 No-op로 둔다.
/// Phase 2에서 tokio-cron-scheduler 백엔드로 교체.
pub struct Scheduler {
    jobs: Vec<Arc<dyn ScheduledJob>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler { jobs: Vec::new() }
    }

    pub fn register(&mut self, job: Arc<dyn ScheduledJob>) {
        self.jobs.push(job);
    }

    pub fn jobs(&self) -> &[Arc<dyn ScheduledJob>] {
        &self.jobs
    }

    /// 등록된 모든 잡을 즉시 1회 실행 (테스트/초기 백필용).
    pub async fn run_all_once(&self) {
        for job in &self.jobs {
            if let Err(e) = job.run().await {
                tracing::warn!(job = %job.name(), error = ?e, "scheduled job failed");
            }
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
