use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;

use crate::error::AppError;
use crate::models::ProvisionDbUserRequest;
use crate::store::Store;

pub fn routes() -> Router<Store> {
    Router::new()
        .route("/", post(provision_db_user).get(list_db_users))
        .route("/{username}", axum::routing::delete(remove_db_user))
}

async fn provision_db_user(
    State(store): State<Store>,
    Json(payload): Json<ProvisionDbUserRequest>,
) -> Result<impl IntoResponse, AppError> {
    let record = store.provision_db_user(payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "data": record
        })),
    ))
}

async fn list_db_users(State(store): State<Store>) -> Result<impl IntoResponse, AppError> {
    let users = store.list_db_users().await?;
    Ok(Json(json!({
        "success": true,
        "count": users.len(),
        "data": users
    })))
}

async fn remove_db_user(
    State(store): State<Store>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let removed = store.remove_db_user(&username).await?;
    Ok(Json(json!({
        "success": removed,
        "message": format!("SurrealDB user '{username}' removed successfully")
    })))
}
