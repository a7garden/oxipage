//! 공통 FTS5 검색 인덱스 (doc/01 §1.7, doc/02 §2.13).
//!
//! 각 확장은 `upsert`/`delete`로 자기 콘텐츠를 `search_documents`에 반영하고,
//! 비활성화 시 `delete_extension`으로 해당 확장의 모든 행을 즉시 동기 삭제한다.

use anyhow::Context;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub extension_id: String,
    pub doc_id: String,
    pub title: String,
    pub snippet: String,
    pub lang: Option<String>,
    pub published_at: Option<String>,
}

/// 발행 시점에 확장이 호출. 같은 (extension_id, doc_id)는 DELETE+INSERT로 upsert.
pub async fn upsert(
    pool: &SqlitePool,
    extension_id: &str,
    doc_id: &str,
    title: &str,
    body: &str,
    lang: Option<&str>,
    published_at: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM search_documents
         WHERE extension_id = ?1 AND doc_id = ?2",
    )
    .bind(extension_id)
    .bind(doc_id)
    .execute(pool)
    .await?;

    sqlix_upsert(pool, extension_id, doc_id, title, body, lang, published_at).await
}

async fn sqlix_upsert(
    pool: &SqlitePool,
    extension_id: &str,
    doc_id: &str,
    title: &str,
    body: &str,
    lang: Option<&str>,
    published_at: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO search_documents
            (extension_id, doc_id, title, body, lang, published_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(extension_id)
    .bind(doc_id)
    .bind(title)
    .bind(body)
    .bind(lang)
    .bind(published_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// 단일 문서 삭제 (초안으로 되돌리거나 게시물 삭제 시).
pub async fn delete(pool: &SqlitePool, extension_id: &str, doc_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM search_documents
         WHERE extension_id = ?1 AND doc_id = ?2",
    )
    .bind(extension_id)
    .bind(doc_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 확장 비활성화 시 즉시 동기 삭제 (doc/02 §2.13).
pub async fn delete_extension(pool: &SqlitePool, extension_id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM search_documents WHERE extension_id = ?1")
        .bind(extension_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `/search?q=` 쿼리. trigram 토크나이저 기반 부분문자열 매칭.
/// 발행된 문서(published_at NOT NULL)만 반환.
pub async fn search(
    pool: &SqlitePool,
    query: &str,
    lang: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<SearchHit>> {
    let pattern = sanitize_fts_query(query);
    if pattern.is_empty() {
        return Ok(Vec::new());
    }

    let rows = if let Some(l) = lang {
        sqlx::query_as::<_, SearchHitRow>(
            "SELECT extension_id, doc_id, title,
                    snippet(search_documents, 3, '«', '»', '…', 16) AS snip,
                    lang, published_at
             FROM search_documents
             WHERE search_documents MATCH ?1
               AND published_at IS NOT NULL
               AND lang = ?2
             ORDER BY rank
             LIMIT ?3",
        )
        .bind(&pattern)
        .bind(l)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, SearchHitRow>(
            "SELECT extension_id, doc_id, title,
                    snippet(search_documents, 3, '«', '»', '…', 16) AS snip,
                    lang, published_at
             FROM search_documents
             WHERE search_documents MATCH ?1
               AND published_at IS NOT NULL
             ORDER BY rank
             LIMIT ?2",
        )
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
    .context("search_documents query failed")?;

    Ok(rows.into_iter().map(SearchHit::from).collect())
}

#[derive(sqlx::FromRow)]
struct SearchHitRow {
    extension_id: String,
    doc_id: String,
    title: String,
    snip: String,
    lang: Option<String>,
    published_at: Option<String>,
}

impl From<SearchHitRow> for SearchHit {
    fn from(r: SearchHitRow) -> Self {
        SearchHit {
            extension_id: r.extension_id,
            doc_id: r.doc_id,
            title: r.title,
            snippet: r.snip,
            lang: r.lang,
            published_at: r.published_at,
        }
    }
}

/// FTS5 쿼리 문자열 정규화. 사용자 입력을 그대로 MATCH에 넣으면 연산자 오류가 발생하므로,
/// 토큰을 큰따옴표로 감싸 phrase 쿼리로 만든다. 빈 토큰은 무시.
fn sanitize_fts_query(input: &str) -> String {
    let mut phrases: Vec<String> = Vec::new();
    for token in input.split_whitespace() {
        if token.is_empty() {
            continue;
        }
        // 큰따옴표 이스케이프: " → "" (FTS5 phrase 안에서)
        let escaped = token.replace('"', "\"\"");
        phrases.push(format!("\"{escaped}\""));
    }
    phrases.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = crate::db::connect_memory().await.unwrap();
        sqlx::query(
            "CREATE VIRTUAL TABLE search_documents USING fts5(
                extension_id UNINDEXED, doc_id UNINDEXED, title, body,
                lang UNINDEXED, published_at UNINDEXED, tokenize='trigram')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[test]
    fn sanitize_handles_operators_and_quotes() {
        assert_eq!(sanitize_fts_query(""), "");
        assert_eq!(sanitize_fts_query("rust"), "\"rust\"");
        assert_eq!(sanitize_fts_query("a\"b"), "\"a\"\"b\"");
        assert_eq!(sanitize_fts_query("rust web"), "\"rust\" \"web\"");
    }

    #[tokio::test]
    async fn upsert_delete_search_roundtrip() {
        let pool = setup().await;
        upsert(
            &pool,
            "blog",
            "hello-rust",
            "Hello Rust",
            "Rust is a systems language",
            Some("en"),
            Some("2026-01-01"),
        )
        .await
        .unwrap();

        let hits = search(&pool, "rust", None, 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].doc_id, "hello-rust");
        assert_eq!(hits[0].extension_id, "blog");

        // draft(published_at NULL)는 검색에서 제외
        upsert(&pool, "blog", "draft", "Draft", "rust draft", None, None)
            .await
            .unwrap();
        let hits = search(&pool, "rust", None, 10).await.unwrap();
        assert_eq!(hits.len(), 1);

        delete(&pool, "blog", "hello-rust").await.unwrap();
        let hits = search(&pool, "rust", None, 10).await.unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn delete_extension_clears_all_rows() {
        let pool = setup().await;
        upsert(&pool, "blog", "a", "A", "body", None, Some("2026-01-01"))
            .await
            .unwrap();
        upsert(&pool, "blog", "b", "B", "body", None, Some("2026-01-01"))
            .await
            .unwrap();
        upsert(&pool, "projects", "c", "C", "body", None, Some("2026-01-01"))
            .await
            .unwrap();

        delete_extension(&pool, "blog").await.unwrap();
        let hits = search(&pool, "body", None, 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].extension_id, "projects");
    }

    #[tokio::test]
    async fn search_filters_by_lang() {
        let pool = setup().await;
        upsert(&pool, "blog", "ko", "Rust", "body", Some("ko"), Some("2026-01-01"))
            .await
            .unwrap();
        upsert(&pool, "blog", "en", "Rust", "body", Some("en"), Some("2026-01-01"))
            .await
            .unwrap();

        let hits = search(&pool, "body", Some("ko"), 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].doc_id, "ko");
    }
}
