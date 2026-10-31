use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;

use crate::error::AppError;
use crate::schema::ensure_schema;
use crate::store::Store;

pub fn routes() -> Router<Store> {
    Router::new().route("/init", post(init_schema))
}

async fn init_schema(State(store): State<Store>) -> Result<impl IntoResponse, AppError> {
    ensure_schema(&store.db).await?;
    Ok(Json(json!({
        "success": true,
        "message": "SurrealDB tables and indexes initialized successfully"
    })))
}
