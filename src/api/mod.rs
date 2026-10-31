pub mod db_users;
pub mod groups;
pub mod schema;
pub mod users;

use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::store::Store;

pub fn build_app(store: Store) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_v1 = Router::new()
        .nest("/users", users::routes())
        .nest("/groups", groups::routes())
        .nest("/db-users", db_users::routes())
        .nest("/schema", schema::routes());

    Router::new()
        .route("/health", get(health_check))
        .nest("/api/v1", api_v1)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(store)
}

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "solid-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
