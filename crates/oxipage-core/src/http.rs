use crate::error::ApiError;
use crate::extension::Lang;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::{Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rust_embed::RustEmbed;
use tower_http::trace::TraceLayer;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct Assets;

#[derive(serde::Serialize)]
struct DataEnvelope<T: serde::Serialize> {
    data: T,
}

#[derive(serde::Serialize)]
struct ManifestSite {
    name: String,
    base_url: String,
    default_lang: String,
    languages: Vec<String>,
}

#[derive(serde::Serialize)]
struct ManifestExtension {
    id: &'static str,
    display_name: ManifestLocalized,
}

#[derive(serde::Serialize)]
struct ManifestLocalized {
    ko: String,
    en: String,
}

#[derive(serde::Serialize)]
struct Manifest {
    site: ManifestSite,
    extensions: Vec<ManifestExtension>,
}

pub fn build_app(state: AppState) -> Router {
    let mut api = Router::new().route("/lobby/manifest", get(lobby_manifest));
    for ext in state.registry.iter() {
        api = api.nest(&format!("/{}", ext.id()), ext.routes());
    }
    let api = api.fallback(api_not_found);

    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v1", api)
        .fallback(static_handler)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn api_not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "resource not found")
}

async fn lobby_manifest(State(state): State<AppState>) -> Json<DataEnvelope<Manifest>> {
    let extensions = state
        .registry
        .iter()
        .map(|e| ManifestExtension {
            id: e.id(),
            display_name: ManifestLocalized {
                ko: e.display_name(Lang::Ko),
                en: e.display_name(Lang::En),
            },
        })
        .collect();
    Json(DataEnvelope {
        data: Manifest {
            site: ManifestSite {
                name: state.config.site.name.clone(),
                base_url: state.config.site.base_url.clone(),
                default_lang: state.config.site.default_lang.clone(),
                languages: state.config.site.languages.clone(),
            },
            extensions,
        },
    })
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    serve_asset(path)
        .or_else(|| serve_asset("index.html"))
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

fn serve_asset(path: &str) -> Option<Response> {
    Assets::get(path).map(|content| {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
    })
}
