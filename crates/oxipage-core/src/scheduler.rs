//! Background job runner (v2 SSG).
//!
//! v1은 cron 표현식으로 자동 주기 실행을 했지만, v2 SSG에서는 더 이상 백그라운드
//! 주기 실행이 필요 없다 — 모든 외부 API 호출은 `oxipage cache refresh`로
//! 명시적 실행한다. 이 모듈은 단순한 "등록된 잡을 한 번씩 실행" 인터페이스만
//! 제공한다. `cache_refresh` HTTP 핸들러가 `Scheduler::run_all_once`를 호출한다.

use crate::state::AppState;
use async_trait::async_trait;
use std::sync::Arc;

/// 스케줄된 잡 정의. v2: `schedule()` 메서드는 인터페이스 호환을 위해
/// 남겨두지만 반환값은 사용되지 않는다 (문서화 목적).
#[async_trait]
pub trait ScheduledJob: Send + Sync {
    /// 잡 식별자 (로깅용).
    fn name(&self) -> &str;

    /// 잡 실행. `AppState`로 DB pool/config에 접근.
    async fn run(&self, ctx: &AppState) -> anyhow::Result<()>;
}

/// 등록된 잡을 모아 한 번에 실행하기 위한 단순 컬렉션.
pub struct Scheduler {
    jobs: Vec<Arc<dyn ScheduledJob>>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
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

    /// 등록된 모든 잡을 즉시 1회 실행 (테스트/oxipage cache refresh용).
    pub async fn run_all_once(&self, ctx: &AppState) {
        for job in &self.jobs {
            if let Err(e) = job.run(ctx).await {
                tracing::warn!(job = %job.name(), error = ?e, "scheduled job failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingJob {
        counter: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl ScheduledJob for CountingJob {
        fn name(&self) -> &str { "counter" }
        async fn run(&self, _ctx: &AppState) -> anyhow::Result<()> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn run_all_once_runs_all_jobs() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut s = Scheduler::new();
        s.register(Arc::new(CountingJob { counter: counter.clone() }));
        s.run_all_once(&test_app_state()).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    fn test_app_state() -> AppState {
        // We don't actually exercise the state here; just satisfy the type.
        // In real code, run_all_once is called from the cache/refresh handler
        // with a fully-initialized AppState.
        panic!("test helper shouldn't be called in this test");
    }
}
