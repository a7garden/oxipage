use crate::model::{Profile, ProfileInput};
use sqlx::SqlitePool;

const COLUMNS: &str = "display_name, tagline_ko, tagline_en, avatar_url, bio_ko, bio_en,
                       email, github_username, linkedin_url, education, custom_links, updated_at";

pub async fn get(pool: &SqlitePool) -> anyhow::Result<Option<Profile>> {
    let profile =
        sqlx::query_as::<_, Profile>(&format!("SELECT {COLUMNS} FROM profile WHERE id = 1"))
            .fetch_optional(pool)
            .await?;
    Ok(profile)
}

pub async fn upsert(pool: &SqlitePool, input: &ProfileInput) -> anyhow::Result<Profile> {
    let education = serde_json::to_string(&input.education)?;
    let custom_links = serde_json::to_string(&input.custom_links)?;
    let profile = sqlx::query_as::<_, Profile>(&format!(
        "INSERT INTO profile (id, display_name, tagline_ko, tagline_en, avatar_url, bio_ko, bio_en,
                              email, github_username, linkedin_url, education, custom_links)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT (id) DO UPDATE SET
            display_name = ?1, tagline_ko = ?2, tagline_en = ?3, avatar_url = ?4,
            bio_ko = ?5, bio_en = ?6, email = ?7, github_username = ?8, linkedin_url = ?9,
            education = ?10, custom_links = ?11,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         RETURNING {COLUMNS}"
    ))
    .bind(&input.display_name)
    .bind(&input.tagline_ko)
    .bind(&input.tagline_en)
    .bind(&input.avatar_url)
    .bind(&input.bio_ko)
    .bind(&input.bio_en)
    .bind(&input.email)
    .bind(&input.github_username)
    .bind(&input.linkedin_url)
    .bind(education)
    .bind(custom_links)
    .fetch_one(pool)
    .await?;
    Ok(profile)
}

/// setup wizard가 form을 받아 부분 업데이트할 때 사용. 빈 문자열은 NULL로 저장.
/// 누락된 필드는 그대로 유지(COALESCE 패턴).
pub async fn update_from_setup_form(
    pool: &SqlitePool,
    form: &serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<()> {
    let get = |k: &str| form.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let dn = get("display_name");
    let tagline_ko = get("tagline_ko");
    let tagline_en = get("tagline_en");
    let github = get("github_username");
    let bio_ko = get("bio_ko");
    let bio_en = get("bio_en");

    sqlx::query(
        "UPDATE profile SET
            display_name = COALESCE(NULLIF(?1, ''), display_name),
            tagline_ko = NULLIF(?2, ''),
            tagline_en = NULLIF(?3, ''),
            github_username = NULLIF(?4, ''),
            bio_ko = NULLIF(?5, ''),
            bio_en = NULLIF(?6, ''),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = 1",
    )
    .bind(dn.unwrap_or_default())
    .bind(tagline_ko.unwrap_or_default())
    .bind(tagline_en.unwrap_or_default())
    .bind(github.unwrap_or_default())
    .bind(bio_ko.unwrap_or_default())
    .bind(bio_en.unwrap_or_default())
    .execute(pool)
    .await?;
    Ok(())
}

/// 싱글턴 행이 없으면 기본값으로 만든다 (서버 부팅 시 호출).
pub async fn ensure_singleton(pool: &SqlitePool, display_name: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT OR IGNORE INTO profile (id, display_name) VALUES (1, ?)")
        .bind(display_name)
        .execute(pool)
        .await?;
    Ok(())
}
