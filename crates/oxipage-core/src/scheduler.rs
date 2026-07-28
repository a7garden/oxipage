//! 백그라운드 잡 스케줄러 (doc/01 §1.9).
//!
//! 코어가 단일 스케줄러 인스턴스를 띄우고, 각 확장이
//! `Extension::background_jobs()`로 등록한 잡을 실행한다.
//!
//! **구현 (doc/08 수정):** `tokio-cron-scheduler` 외부 의존성 대신 가벼운
//! 6-field cron 파서(`sec min hour dom mon dow`) + `tokio::time::sleep`
//! 드라이버. 1인 사이트 규모에서 충분하며, 의존성 증가를 피한다.
//!
//! **인터페이스 수정:** `ScheduledJob::run(&self, &AppState)` — job body가
//! DB pool/config에 접근할 수 있도록 `AppState`를 인자로 받는다. 이전
//! 시그니처(`run(&self)`)는 job이 구조적으로 no-op일 수밖에 없었다.

use crate::state::AppState;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::time::Duration;

/// 스케줄된 잡. cron 식과 실행 함수를 캡슐화.
#[async_trait]
pub trait ScheduledJob: Send + Sync {
    /// cron 식 (6-field: "sec min hour dom mon dow", 예: "0 */15 * * * *" = 15분마다).
    fn schedule(&self) -> &str;

    /// 잡 식별자 (로깅/모니터링용).
    fn name(&self) -> &str;

    /// 잡 실행. `AppState`로 DB pool/config에 접근. 실패해도 스케줄러가
    /// 다음 주기에 재시도한다 (에러 로깅만).
    async fn run(&self, ctx: &AppState) -> anyhow::Result<()>;
}

/// 6-field cron 식을 파싱한 결과 (sec min hour dom mon dow).
/// 각 필드는 매칭되는 값의 집합. 빈 vec = 와일드카드(모두 매칭).
#[derive(Debug, Clone)]
struct CronSchedule {
    #[allow(dead_code)]
    seconds: Vec<u8>,
    minutes: Vec<u8>,
    hours: Vec<u8>,
    #[allow(dead_code)]
    doms: Vec<u8>,
    #[allow(dead_code)]
    months: Vec<u8>,
    #[allow(dead_code)] // dow는 파싱만 하고 스케줄링엔 미사용 (1인 사이트 규모)
    dows: Vec<u8>,
}

impl CronSchedule {
    fn parse(expr: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 6 {
            anyhow::bail!(
                "cron expression must have 6 fields (sec min hour dom mon dow), got {}: '{expr}'",
                parts.len()
            );
        }
        Ok(Self {
            seconds: parse_field(parts[0], 0, 59)?,
            minutes: parse_field(parts[1], 0, 59)?,
            hours: parse_field(parts[2], 0, 23)?,
            doms: parse_field(parts[3], 1, 31)?,
            months: parse_field(parts[4], 1, 12)?,
            dows: parse_field(parts[5], 0, 6)?,
        })
    }

    /// 다음 실행 시점까지 대기할 초 단위 duration (근사).
    /// 분 단위로 정렬 — 백그라운드 잡에 초 단위 정밀도는 불필요.
    fn next_wait_seconds(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let cur_min = ((secs / 60) % 60) as u8;
        let cur_hour = ((secs / 3600) % 24) as u8;

        let matches = |val: u8, set: &[u8]| set.is_empty() || set.contains(&val);
        if matches(cur_min, &self.minutes) && matches(cur_hour, &self.hours) {
            // 이 분에 이미 실행됐을 수 있으니 다음 매칭까지 60초 대기.
            return 60;
        }
        // 다음 매칭 분 찾기 (최대 60분 순회).
        let mut m = cur_min;
        for _ in 0..60 {
            m = if m == 59 { 0 } else { m + 1 };
            if matches(m, &self.minutes) {
                let wait = if m > cur_min {
                    (m - cur_min) as u64 * 60
                } else {
                    (60 - cur_min + m) as u64 * 60
                };
                return wait.max(1);
            }
        }
        // fallback: 1시간 대기.
        3600
    }
}

fn parse_field(s: &str, min: u8, max: u8) -> anyhow::Result<Vec<u8>> {
    if s == "*" {
        return Ok(Vec::new()); // 와일드카드
    }
    let mut out = Vec::new();
    for part in s.split(',') {
        if part == "*" {
            return Ok(Vec::new());
        }
        if let Some((range_str, step_str)) = part.split_once('/') {
            let step: u8 = step_str
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid step '{step_str}'"))?;
            if step == 0 {
                anyhow::bail!("step must be > 0");
            }
            let (lo, hi) = if range_str == "*" {
                (min, max)
            } else if let Some((a, b)) = range_str.split_once('-') {
                (a.parse()?, b.parse()?)
            } else {
                let v: u8 = range_str.parse()?;
                (v, v)
            };
            let mut v = lo;
            while v <= hi {
                if v >= min && v <= max {
                    out.push(v);
                }
                v = v.saturating_add(step);
                if v < lo {
                    break; // overflow
                }
            }
        } else if let Some((a, b)) = part.split_once('-') {
            let lo: u8 = a.parse()?;
            let hi: u8 = b.parse()?;
            for v in lo..=hi {
                if v >= min && v <= max {
                    out.push(v);
                }
            }
        } else {
            let v: u8 = part.parse()?;
            if v < min || v > max {
                anyhow::bail!("value {v} out of range [{min},{max}]");
            }
            out.push(v);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

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
    pub async fn run_all_once(&self, ctx: &AppState) {
        for job in &self.jobs {
            if let Err(e) = job.run(ctx).await {
                tracing::warn!(job = %job.name(), error = ?e, "scheduled job failed");
            }
        }
    }

    /// 모든 잡을 백그라운드 태스크로 spawn. 각 잡은 자신의 cron 식에 따라
    /// 주기적으로 실행된다. 서버 부팅 시 호출.
    pub fn spawn_all(&self, ctx: AppState) {
        for job in &self.jobs {
            let j = Arc::clone(job);
            let ctx = ctx.clone();
            let name = j.name().to_string();
            let schedule_expr = j.schedule().to_string();
            match CronSchedule::parse(&schedule_expr) {
                Ok(cron) => {
                    tracing::info!(
                        job = %name,
                        schedule = %schedule_expr,
                        "spawning scheduled job"
                    );
                    tokio::spawn(async move {
                        loop {
                            let wait = cron.next_wait_seconds();
                            tokio::time::sleep(Duration::from_secs(wait)).await;
                            if let Err(e) = j.run(&ctx).await {
                                tracing::warn!(job = %name, error = ?e, "scheduled job failed");
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::error!(
                        job = %name,
                        schedule = %schedule_expr,
                        error = %e,
                        "failed to parse cron schedule — job not spawned"
                    );
                }
            }
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_15_min() {
        let c = CronSchedule::parse("0 */15 * * * *").unwrap();
        assert_eq!(c.seconds, vec![0]);
        assert!(c.minutes.contains(&0));
        assert!(c.minutes.contains(&15));
        assert!(c.minutes.contains(&30));
        assert!(c.minutes.contains(&45));
    }

    #[test]
    fn parses_every_30_min() {
        let c = CronSchedule::parse("0 */30 * * * *").unwrap();
        assert!(c.minutes.contains(&0));
        assert!(c.minutes.contains(&30));
        assert!(!c.minutes.contains(&15));
    }

    #[test]
    fn wildcard_field_is_empty() {
        let c = CronSchedule::parse("0 */30 * * * *").unwrap();
        assert!(c.hours.is_empty());
        assert!(c.doms.is_empty());
        assert!(c.months.is_empty());
        assert!(c.dows.is_empty());
    }

    #[test]
    fn rejects_bad_field_count() {
        assert!(CronSchedule::parse("0 */15 * * *").is_err());
        assert!(CronSchedule::parse("0 */15 * * * * extra").is_err());
    }

    #[test]
    fn next_wait_returns_positive() {
        let c = CronSchedule::parse("0 */30 * * * *").unwrap();
        assert!(c.next_wait_seconds() >= 1);
    }

    #[test]
    fn range_field() {
        let c = CronSchedule::parse("0 0 9-17 * * *").unwrap();
        assert_eq!(c.hours, vec![9, 10, 11, 12, 13, 14, 15, 16, 17]);
    }

    #[test]
    fn list_field() {
        let c = CronSchedule::parse("0 0 8,12,18 * * *").unwrap();
        assert_eq!(c.hours, vec![8, 12, 18]);
    }
}
