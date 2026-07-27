pub mod client;
pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::state::AppState;

pub struct BooksExtension;

#[async_trait]
impl Extension for BooksExtension {
    fn id(&self) -> &'static str {
        "books"
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "책".to_string(),
            Lang::En => "Books".to_string(),
        }
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration {
            version: 1,
            name: "init",
            sql: include_str!("../migrations/0001_init.sql"),
        }]
    }

    fn routes(&self) -> Router<AppState> {
        // 라우트 등록 순서:
        //   - `/{id}` (정수) 파라미터는 `Path<i64>`로 추출되므로 `/search` 같은
        //     정수가 아닌 경로는 자동으로 미스매치된다. 단, axum 0.8의 매처는
        //     리터럴을 우선 매칭하므로 `/search` 등록 순서는 안전하다.
        //   - 메서드 라우팅(POST/PATCH/DELETE) 충돌은 `/{id}` 라인에 묶어서
        //     단일 핸들러당 단일 HTTP 메서드만 매핑한다.
        Router::new()
            .route("/", get(routes::list).post(routes::create))
            // 외부 도서 검색: 알라딘 → Google Books → manual 안내.
            // 503 (book_search_disabled)은 양쪽 키가 모두 없을 때만.
            .route("/search", get(routes::external_search))
            .route(
                "/{id}",
                get(routes::show)
                    .patch(routes::update)
                    .delete(routes::delete),
            )
            .route("/{id}/publish", post(routes::publish))
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let recent = repo::list(&ctx.db, None, 3).await.ok()?;
        if recent.is_empty() {
            return None;
        }
        let items = recent
            .into_iter()
            .map(|b| LobbyCardItem {
                title: b.title,
                url: format!("/books/{}", b.id),
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items,
        })
    }
}
