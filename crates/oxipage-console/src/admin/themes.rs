//! 테마 카탈로그.
//!
//! 큐레이션된 테마 프리셋 목록. Admin SPA가 이 카탈로그를 렌더링하고,
//! 선택된 테마는 `PUT /api/console/theme`으로 사이트 서버에 저장한다 (proxy 통해).

use super::AdminContext;
use axum::extract::State;
use axum::Json;
use serde::Serialize;

/// 단일 테마 엔트리.
#[derive(Debug, Serialize)]
pub struct ThemeEntry {
    pub id: &'static str,
    pub name_ko: &'static str,
    pub name_en: &'static str,
    pub mode: &'static str,
    pub accent_hue: f64,
    pub description_ko: &'static str,
    pub description_en: &'static str,
    /// 미리보기 컬러 4개 (canvas, surface, text, accent hex approximations).
    pub preview_colors: [&'static str; 4],
}

/// 큐레이션 테마 목록 (v1: 4종).
const THEMES: &[ThemeEntry] = &[
    ThemeEntry {
        id: "paper",
        name_ko: "종이",
        name_en: "Paper",
        mode: "light",
        accent_hue: 290.0,
        description_ko: "따뜻한 종이 배경, 인디고-바이올렛 악센트",
        description_en: "Warm paper background, indigo-violet accent",
        preview_colors: ["#fafaf5", "#f5f2ed", "#2d2934", "#5e3cbd"],
    },
    ThemeEntry {
        id: "midnight",
        name_ko: "한밤",
        name_en: "Midnight",
        mode: "dark",
        accent_hue: 230.0,
        description_ko: "깊은 밤하늘, 시안-블루 악센트",
        description_en: "Deep night sky, cyan-blue accent",
        preview_colors: ["#1a1a2e", "#16213e", "#e0e0e0", "#4fc3f7"],
    },
    ThemeEntry {
        id: "sepia",
        name_ko: "세피아",
        name_en: "Sepia",
        mode: "light",
        accent_hue: 70.0,
        description_ko: "오래된 책장, 앰버-골드 악센트",
        description_en: "Old bookshelf, amber-gold accent",
        preview_colors: ["#f5f0e8", "#ede0d4", "#3d3529", "#b8860b"],
    },
    ThemeEntry {
        id: "forest",
        name_ko: "숲",
        name_en: "Forest",
        mode: "dark",
        accent_hue: 155.0,
        description_ko: "이끼 낀 숲, 에메랄드 악센트",
        description_en: "Mossy forest, emerald accent",
        preview_colors: ["#1b2b1b", "#243624", "#e0e8e0", "#2ecc71"],
    },
];

/// GET /api/admin/themes — 큐레이션 테마 카탈로그 반환
pub(crate) async fn catalog_handler(
    State(_ctx): State<AdminContext>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({"data": THEMES}))
}

/// 테마 ID 유효성 검증. 유효하면 Some, 아니면 None.
pub fn validate_theme_id(id: &str) -> Option<&'static ThemeEntry> {
    THEMES.iter().find(|t| t.id == id)
}
