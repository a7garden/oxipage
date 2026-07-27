//! 레이트리밋 (doc/04 §4.5 — 공개 읽기 API는 레이트리밋만 적용).
//!
//! 1인 사이트 규모용 간단한 in-memory IP 토큰 버킷. 외부 의존성(governor) 없이
//! 구현. 분산 환경에서는 Redis 백엔드로 교체 여지 (v1 제외).
//!
//! 정책: IP당 `capacity` 토큰, `refill_per_sec` 토큰/초 보충. 기본 60/분.

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<HashMap<IpAddr, Bucket>>>,
    capacity: f64,
    refill_per_sec: f64,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(capacity_per_min: u32) -> Self {
        let capacity = capacity_per_min as f64;
        RateLimiter {
            inner: Arc::new(Mutex::new(HashMap::new())),
            capacity,
            refill_per_sec: capacity / 60.0,
        }
    }

    fn allow(&self, ip: IpAddr) -> bool {
        let mut map = self.inner.lock().unwrap();
        let now = Instant::now();
        let bucket = map.entry(ip).or_insert(Bucket {
            tokens: self.capacity,
            last: now,
        });
        let elapsed = now.duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        bucket.last = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// axum 미들웨어. X-Forwarded-For(신뢰하는 프록시 뒤 단정) 또는 연결 IP.
pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(ip) = client_ip(&req)
        && !limiter.allow(ip)
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    Ok(next.run(req).await)
}

fn client_ip(req: &Request) -> Option<IpAddr> {
    if let Some(xff) = req.headers().get("x-forwarded-for")
        && let Ok(s) = xff.to_str()
        && let Some(first) = s.split(',').next()
    {
        if let Ok(ip) = first.trim().parse::<IpAddr>() {
            return Some(ip);
        }
    }
    req.extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_until_capacity_then_denies() {
        let limiter = RateLimiter::new(3);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(limiter.allow(ip));
        assert!(limiter.allow(ip));
        assert!(limiter.allow(ip));
        assert!(!limiter.allow(ip));
    }

    #[test]
    fn different_ips_independent() {
        let limiter = RateLimiter::new(1);
        let a: IpAddr = "127.0.0.1".parse().unwrap();
        let b: IpAddr = "127.0.0.2".parse().unwrap();
        assert!(limiter.allow(a));
        assert!(limiter.allow(b));
        assert!(!limiter.allow(a));
    }
}
