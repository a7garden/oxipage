//! Tests for the media upload + serve endpoints.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxibuilder_console::router::build_console_router;
use oxibuilder_console::sites_runtime::SiteRegistry;
use oxibuilder_core::sites::SitesFile;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

fn minimal_toml(name: &str) -> String {
    format!(
        r#"[site]
name = "{name}"
base_url = "http://127.0.0.1:8787"
default_lang = "ko"
languages = ["ko"]

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"
"#,
    )
}

async fn build_app() -> (TempDir, Router) {
    let dir = TempDir::with_prefix("oxibuilder-media-").unwrap();
    let toml_path = dir.path().join("oxibuilder.toml");
    std::fs::write(&toml_path, minimal_toml("Test")).unwrap();
    let mut sf = SitesFile::default();
    sf.add("blog".into(), dir.path().to_path_buf());
    sf.set_default("blog");
    let registry = Arc::new(SiteRegistry::new(sf, Default::default()).await.unwrap());
    (dir, build_console_router(registry))
}

// 1×1 transparent PNG (smallest valid PNG).
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

fn build_multipart(filename: &str, content: &[u8]) -> (String, Vec<u8>) {
    let boundary = "----oxibuilder-test-boundary";
    let body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{f}\"\r\nContent-Type: image/png\r\n\r\n",
        b = boundary,
        f = filename,
    )
    .into_bytes();
    let mut full = body;
    full.extend_from_slice(content);
    full.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let ct = format!("multipart/form-data; boundary={boundary}");
    (ct, full)
}

#[tokio::test]
async fn upload_png_round_trips() {
    let (_dir, app) = build_app().await;
    let (ct, body) = build_multipart("avatar.png", PNG_1X1);
    let req = Request::builder()
        .method("POST")
        .uri("/s/blog/media/profile")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let path = json["data"]["path"].as_str().unwrap().to_string();
    assert!(path.starts_with("media/profile/"), "path: {path}");
    assert!(path.ends_with(".png"), "path: {path}");
    assert_eq!(json["data"]["mime"], "image/png");

    let file_name = path.rsplit('/').next().unwrap();
    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/s/blog/media/profile/{file_name}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(get_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let back = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    assert_eq!(back.as_ref(), PNG_1X1);
}

#[tokio::test]
async fn upload_rejects_fake_png() {
    let (_dir, app) = build_app().await;
    let (ct, body) = build_multipart("avatar.png", b"not an image at all");
    let req = Request::builder()
        .method("POST")
        .uri("/s/blog/media/profile")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn upload_rejects_oversize() {
    let (_dir, app) = build_app().await;
    let mut payload = PNG_1X1.to_vec();
    payload.resize(11 * 1024 * 1024, 0);
    let (ct, body) = build_multipart("huge.png", &payload);
    let req = Request::builder()
        .method("POST")
        .uri("/s/blog/media/profile")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn upload_rejects_invalid_extension_id() {
    let (_dir, app) = build_app().await;
    let (ct, body) = build_multipart("avatar.png", PNG_1X1);
    let req = Request::builder()
        .method("POST")
        .uri("/s/blog/media/..%2fetc")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::NOT_FOUND);
}

async fn read_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn list_media_after_upload() {
    let (_dir, app) = build_app().await;
    let (ct, body) = build_multipart("a.png", PNG_1X1);
    let req = Request::builder()
        .method("POST")
        .uri("/s/blog/media/profile")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let up = read_json(resp).await;
    let path = up["data"]["path"].as_str().unwrap().to_string();

    let req = Request::builder()
        .method("GET")
        .uri("/s/blog/media")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list = read_json(resp).await;
    let items = list["data"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["path"], path);
    assert_eq!(items[0]["extension"], "profile");
    assert!(items[0]["bytes"].as_u64().unwrap() > 0);
    assert!(items[0]["updated_at"].as_str().unwrap().ends_with('Z'));

    // extension filter excludes other namespaces
    let req = Request::builder()
        .method("GET")
        .uri("/s/blog/media?extension=blog")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let list2 = read_json(resp).await;
    assert!(list2["data"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn delete_media_removes_file() {
    let (_dir, app) = build_app().await;
    let (ct, body) = build_multipart("a.png", PNG_1X1);
    let req = Request::builder()
        .method("POST")
        .uri("/s/blog/media/profile")
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let up = read_json(resp).await;
    let path = up["data"]["path"].as_str().unwrap().to_string();
    let file = path.rsplit('/').next().unwrap().to_string();

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/s/blog/media/profile/{file}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // serve now 404
    let req = Request::builder()
        .method("GET")
        .uri(format!("/s/blog/media/profile/{file}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_rejects_traversal() {
    let (_dir, app) = build_app().await;
    let req = Request::builder()
        .method("DELETE")
        .uri("/s/blog/media/profile/..%2F..%2Fetc%2Fpasswd")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert!(
        resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::NOT_FOUND,
        "status: {}",
        resp.status()
    );
}
