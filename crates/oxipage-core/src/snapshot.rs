//! SSR 스냅샷 (doc/01 §1.6).
//!
//! 발행 시점에 prerendered HTML을 `/data/snapshots/`에 저장. OG 메타 + canonical +
//! 본문 텍스트 + 동일 SPA 스크립트 태그. 봇(Slack/카카오톡/Twitter)과 대부분의
//! 크롤러는 이 스냅샷을 받고, 브라우저는 같은 HTML을 하이드레이트.
//!
//! **편차 (doc/01 §1.6 대비):** Askama 대신 수동 `format!` 템플릿. 의존성 절약.
//! 마크다운 → HTML은 프론트가 담당; 스냅샷 본문은 마크다운 원문을 `<main>`에 직접
//! (봇이 텍스트로 읽음), SPA가 하이드레이트 시 교체.

use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct SnapshotData {
    pub title: String,
    pub description: String,
    pub canonical_url: String,
    pub og_image: Option<String>,
    /// 발행 콘텐츠의 마크다운 원문 (또는 요약 텍스트). 봇 가독용.
    pub body_markdown: String,
    pub lang: String,
}

/// prerendered HTML 생성. `spa_asset`는 SPA 진입 스크립트 경로.
pub fn render(data: &SnapshotData, spa_asset: &str) -> String {
    let og_image = data
        .og_image
        .as_ref()
        .map(|url| format!("<meta property=\"og:image\" content=\"{url}\">"))
        .unwrap_or_default();
    let desc = html_escape(&data.description);
    let title = html_escape(&data.title);
    let body = html_escape(&data.body_markdown);
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<meta name="description" content="{desc}">
<link rel="canonical" href="{canonical}">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{desc}">
<meta property="og:type" content="article">
<meta property="og:url" content="{canonical}">
{og_image}
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{title}">
<meta name="twitter:description" content="{desc}">
</head>
<body>
<div id="root">
<main>{body}</main>
<noscript>{title}</noscript>
</div>
<script type="module" src="{spa}"></script>
</body>
</html>
"#,
        lang = data.lang,
        title = title,
        desc = desc,
        canonical = html_escape(&data.canonical_url),
        og_image = og_image,
        body = body,
        spa = spa_asset,
    )
}

/// `index.html`을 베이스로 OG 메타/canonical/본문을 주입한 SSR 출력.
///
/// - `<title>…</title>` 을 `data.title` 로 교체.
/// - `</head>` 직전에 OG 메타 + canonical + description 부착.
/// - `<div id="root"></div>` 직후에 `<main data-snapshot="true">…</main>` 추가.
///
/// 브라우저는 같은 HTML을 받아 React가 `#root` 안의 `main` 요소를 교체한다.
pub fn render_with_index(index_html: &str, data: &SnapshotData) -> String {
    let og_image = data
        .og_image
        .as_deref()
        .map(|url| format!("<meta property=\"og:image\" content=\"{}\">", html_escape(url)))
        .unwrap_or_default();
    let title = html_escape(&data.title);
    let desc = html_escape(&data.description);
    let canonical = html_escape(&data.canonical_url);
    let body = html_escape(&data.body_markdown);

    let meta_block = format!(
        "<meta name=\"description\" content=\"{desc}\">\n\
         <link rel=\"canonical\" href=\"{canonical}\">\n\
         <meta property=\"og:title\" content=\"{title}\">\n\
         <meta property=\"og:description\" content=\"{desc}\">\n\
         <meta property=\"og:type\" content=\"article\">\n\
         <meta property=\"og:url\" content=\"{canonical}\">\n\
         {og_image}\n\
         <meta name=\"twitter:card\" content=\"summary_large_image\">\n\
         <meta name=\"twitter:title\" content=\"{title}\">\n\
         <meta name=\"twitter:description\" content=\"{desc}\">"
    );

    // 1) <title>…</title> 교체 (첫 번째 등장분만).
    let mut html = if let Some(start) = index_html.find("<title>") {
        if let Some(end_rel) = index_html[start..].find("</title>") {
            let end = start + end_rel + "</title>".len();
            let mut owned = index_html.to_owned();
            owned.replace_range(start..end, &format!("<title>{title}</title>"));
            owned
        } else {
            index_html.to_owned()
        }
    } else {
        index_html.to_owned()
    };

    // 2) </head> 직전에 meta_block 주입.
    if let Some(pos) = html.find("</head>") {
        html.insert_str(pos, &meta_block);
        html.insert(pos, '\n');
    }

    // 3) <div id="root"> 뒤에 SSR 본문 삽입 (React 하이드레이트 시 이 main을 교체).
    let marker = "<main data-snapshot=\"true\">";
    let insert_html = format!(
        "\n{marker}{body}</main>\n",
        marker = marker,
        body = body
    );
    if let Some(pos) = html.find(r#"<div id="root">"#) {
        // root div 바로 뒤(=root의 자식 시작점)에 추가.
        let after = pos + r#"<div id="root">"#.len();
        html.insert_str(after, &insert_html);
    }

    html
}

/// SSR 스냅샷 작성 helper. `index.html` 로드가 가능하고 publish가 의미를 가질 때만 동작.
/// best-effort: 실패해도 호출자 흐름을 막지 않도록 `Result`를 무시할 수 있도록 tracing만.
pub async fn write_snapshot_for(ctx: &AppState, path: &str, data: &SnapshotData) {
    let Some(index_html) = crate::http::spa_index_html() else {
        tracing::debug!(path, "spa_index_html unavailable; skip snapshot");
        return;
    };
    let html = render_with_index(&index_html, data);
    if let Err(e) = write_snapshot(ctx, path, &html).await {
        tracing::warn!(error = ?e, path, "failed to write SSR snapshot");
    }
}
/// `/data/snapshots/<safe-path>.html`에 스냅샷 저장.
pub async fn write_snapshot(ctx: &AppState, path: &str, html: &str) -> anyhow::Result<()> {
    let dir = ctx.config.server.data_dir.join("snapshots");
    let file_name = sanitize_snapshot_path(path);
    let file = dir.join(format!("{file_name}.html"));
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&file, html).await?;
    tracing::debug!(snapshot = %file.display(), "wrote SSR snapshot");
    Ok(())
}

/// 발행 취소/삭제 시 스냅샷 제거.
pub async fn remove_snapshot(ctx: &AppState, path: &str) -> anyhow::Result<()> {
    let dir = ctx.config.server.data_dir.join("snapshots");
    let file_name = sanitize_snapshot_path(path);
    let file = dir.join(format!("{file_name}.html"));
    if file.exists() {
        tokio::fs::remove_file(&file).await?;
    }
    Ok(())
}

/// URL 경로 → 안전한 파일명. traversal 세그먼트(.., .) 제거 + 세그먼트 결합.
fn sanitize_snapshot_path(path: &str) -> String {
    path.trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".." && *s != ".")
        .map(|seg| {
            seg.chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("_")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_contains_og_and_canonical() {
        let data = SnapshotData {
            title: "Hello <World>".into(),
            description: "desc & more".into(),
            canonical_url: "https://x.com/blog/hello".into(),
            og_image: Some("https://x.com/img.png".into()),
            body_markdown: "# Hello".into(),
            lang: "ko".into(),
        };
        let html = render(&data, "/assets/index.js");
        assert!(html.contains("og:title"));
        assert!(html.contains("og:image"));
        assert!(html.contains("rel=\"canonical\""));
        assert!(html.contains("&lt;World&gt;"));
        assert!(html.contains("desc &amp; more"));
        assert!(html.contains("/assets/index.js"));
    }

    #[test]
    fn sanitize_strips_traversal() {
        assert_eq!(sanitize_snapshot_path("/blog/hello"), "blog_hello");
        assert_eq!(sanitize_snapshot_path("/blog/../etc"), "blog_etc");
        assert_eq!(sanitize_snapshot_path("blog/hello-world"), "blog_hello-world");
    }

    #[test]
    fn render_without_og_image() {
        let data = SnapshotData {
            title: "T".into(),
            description: "D".into(),
            canonical_url: "https://x.com/".into(),
            og_image: None,
            body_markdown: "".into(),
            lang: "en".into(),
        };
        let html = render(&data, "/a.js");
        assert!(!html.contains("og:image"));
    }

    #[test]
    fn render_with_index_replaces_title_and_injects_meta_and_main() {
        let index = r#"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="utf-8">
<title>Old title</title>
<script type="module" src="/assets/index.js"></script>
</head>
<body>
<div id="root"></div>
</body>
</html>"#;
        let data = SnapshotData {
            title: "New <Title>".into(),
            description: "desc & <tag>".into(),
            canonical_url: "https://x.com/blog/hello".into(),
            og_image: Some("https://x.com/img.png".into()),
            body_markdown: "# Body\n\ntext".into(),
            lang: "ko".into(),
        };
        let out = render_with_index(index, &data);
        assert!(out.contains("<title>New &lt;Title&gt;</title>"));
        assert!(!out.contains("Old title"));
        assert!(out.contains(r#"<meta name="description" content="desc &amp; &lt;tag&gt;">"#));
        assert!(out.contains(r#"<link rel="canonical" href="https://x.com/blog/hello">"#));
        assert!(out.contains(r#"<meta property="og:image" content="https://x.com/img.png">"#));
        assert!(out.contains(r#"<meta property="og:url" content="https://x.com/blog/hello">"#));
        // main은 div#root 직후 어딘가에 있고, data-snapshot 마커가 있다 (정확한 공백은 format! 결과에 의존).
        assert!(out.contains(r#"<main data-snapshot="true">"#));
        assert!(out.contains("# Body"));
        assert!(out.contains("</main>"));
    }

    #[test]
    fn render_with_index_skips_og_image_when_none() {
        let index = r#"<!DOCTYPE html><html><head><title>t</title></head><body><div id="root"></div></body></html>"#;
        let data = SnapshotData {
            title: "T".into(),
            description: "d".into(),
            canonical_url: "https://x.com/".into(),
            og_image: None,
            body_markdown: "".into(),
            lang: "en".into(),
        };
        let out = render_with_index(index, &data);
        assert!(!out.contains("og:image"));
        assert!(out.contains("rel=\"canonical\""));
    }
}
