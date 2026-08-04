//! 별점 값 객체 (doc/02 §2.1)
//!
//! **저장 계약 (모순 최소 해석):** `0~10` 정수로 저장.
//!   - "0.5점 단위를 반정수로 표현" → 정수 1스텝 = 0.5점 (0, 1, 2, …, 10)
//!   - 프론트에서 `/2.0`으로 환산 → `0.0~5.0`점
//!   - 별 5개 = 5.0점 만점
//!
//! doc/02 본문의 "0~20 정수"는 "/2.0 환산 · 최대 5.0점 별 5개"와 함께 읽으면 모순이므로
//! 오타로 판정해 `0~10`을 정규 계약으로 채택한다. 모든 movies/books 확장과 프론트 별
//! 컴포넌트가 이 계약에 묶인다.
//!
//! DB 칼럼은 INTEGER로 저장. 모델 struct에서는 `i8`로 읽고 애플리케이션에서
//! `Rating::from_raw`로 래핑한다. 단순함을 위해 sqlx 커스텀 타입은 두지 않는다.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Rating(pub i8);

impl Rating {
    pub const MAX: i8 = 10;
    pub const MIN: i8 = 0;

    /// 0~10 범위 검증 후 래핑. 범위 밖이면 Err.
    pub fn new(value: i8) -> Result<Self, RatingError> {
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(RatingError::OutOfRange { value });
        }
        Ok(Rating(value))
    }

    /// 범위 검증 없이 래핑 (DB에서 읽은 값 등 이미 검증된 경우).
    pub const fn from_raw(value: i8) -> Self {
        Rating(value)
    }

    /// 0.0~5.0점 실수값 (스펙: `/2.0` 환산).
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / 2.0
    }

    pub const fn raw(self) -> i8 {
        self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RatingError {
    #[error("rating out of range (0..=10): got {value}")]
    OutOfRange { value: i8 },
}

impl Serialize for Rating {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i8(self.0)
    }
}

impl<'de> Deserialize<'de> for Rating {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = i8::deserialize(deserializer)?;
        Rating::new(v).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_out_of_range() {
        assert!(Rating::new(-1).is_err());
        assert!(Rating::new(0).is_ok());
        assert!(Rating::new(10).is_ok());
        assert!(Rating::new(11).is_err());
        assert!(Rating::new(20).is_err());
    }

    #[test]
    fn to_f32_maps_to_zero_to_five() {
        assert_eq!(Rating::new(0).unwrap().to_f32(), 0.0);
        assert_eq!(Rating::new(2).unwrap().to_f32(), 1.0);
        assert!((Rating::new(10).unwrap().to_f32() - 5.0).abs() < f32::EPSILON);
    }
}
