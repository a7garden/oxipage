//! Shared server-side validators. Single source of truth mirrored by
//! `web/src/admin/shared/validation.ts`.

pub fn is_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

pub fn is_media_path(s: &str) -> bool {
    if s.starts_with('/') || s.starts_with('.') || s.contains("..") {
        return false;
    }
    if s.starts_with("javascript:") || s.starts_with("data:") || s.starts_with("file:") {
        return false;
    }
    let mut parts = s.splitn(3, '/');
    let kind = parts.next();
    let ext = parts.next();
    let file = parts.next();
    let valid_kind = matches!(kind, Some("media"));
    let valid_ext = ext
        .map(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(false);
    let valid_file = file
        .map(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'))
        .unwrap_or(false);
    valid_kind && valid_ext && valid_file
}

pub fn is_image_value(s: &str) -> bool {
    is_http_url(s) || is_media_path(s)
}

pub fn clamp_rating(v: i8) -> Option<i8> {
    if (0..=10).contains(&v) {
        Some(v)
    } else {
        None
    }
}

pub fn validate_year(v: i32) -> Option<i32> {
    if (1000..=9999).contains(&v) {
        Some(v)
    } else {
        None
    }
}

pub fn validate_email(s: &str) -> bool {
    let mut at = s.split('@');
    let local = at.next().unwrap_or("");
    let domain = at.next().unwrap_or("");
    let mut dp = domain.split('.');
    let d0 = dp.next().unwrap_or("");
    let d1 = dp.next().unwrap_or("");
    !local.is_empty() && !d0.is_empty() && !d1.is_empty() && !s.contains(' ')
}

pub fn validate_isbn13(s: &str) -> bool {
    let t: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if t.len() != 13 {
        return false;
    }
    let mut sum = 0i32;
    for (i, ch) in t.chars().enumerate().take(12) {
        let d = ch.to_digit(10).unwrap() as i32;
        sum += if i % 2 == 0 { d } else { d * 3 };
    }
    let check = (10 - (sum % 10)) % 10;
    check == t.chars().last().unwrap().to_digit(10).unwrap() as i32
}

pub fn validate_date_order(start: Option<&str>, end: Option<&str>) -> bool {
    match (start, end) {
        (Some(s), Some(e)) if !s.is_empty() && !e.is_empty() => s <= e,
        _ => true,
    }
}

pub fn validate_year_order(start: Option<i32>, end: Option<i32>) -> bool {
    match (start, end) {
        (Some(s), Some(e)) => s <= e,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert!(is_http_url("https://x"));
        assert!(!is_http_url("ftp://x"));
    }

    #[test]
    fn media() {
        assert!(is_media_path("media/profile/a.webp"));
        assert!(!is_media_path("/media/x"));
        assert!(!is_media_path("media/x/../y"));
        assert!(!is_media_path("javascript:alert(1)"));
    }

    #[test]
    fn isbn() {
        assert!(validate_isbn13("9780306406157"));
        assert!(!validate_isbn13("9780306406150"));
    }

    #[test]
    fn email() {
        assert!(validate_email("a@b.co"));
        assert!(!validate_email("a@b"));
    }

    #[test]
    fn date() {
        assert!(validate_date_order(Some("2024-01-01"), Some("2024-02-01")));
        assert!(!validate_date_order(Some("2024-02-01"), Some("2024-01-01")));
        assert!(validate_date_order(None, Some("2024-01-01")));
    }

    #[test]
    fn year_order() {
        assert!(validate_year_order(Some(2020), Some(2024)));
        assert!(!validate_year_order(Some(2024), Some(2020)));
    }
}
