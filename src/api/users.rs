use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::error::AppError;
use crate::models::{AssignPlatformRoleRequest, ProvisionUserRequest, UpdateUserRequest};
use crate::store::Store;

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub limit: Option<usize>,
}

pub fn routes() -> Router<Store> {
    Router::new()
        .route("/", post(provision_user).get(list_users))
        .route(
            "/{sub_or_username}",
            get(get_user).put(update_user).delete(delete_user),
        )
        .route("/{sub_or_username}/role", post(assign_role))
        .route("/{sub_or_username}/access", get(get_access_context))
        .route("/{sub_or_username}/groups", get(get_user_groups))
}

async fn provision_user(
    State(store): State<Store>,
    Json(payload): Json<ProvisionUserRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = store.provision_user(payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "data": user
        })),
    ))
}

async fn list_users(
    State(store): State<Store>,
    Query(params): Query<ListUsersQuery>,
) -> Result<impl IntoResponse, AppError> {
    let users = store.list_users(params.limit).await?;
    Ok(Json(json!({
        "success": true,
        "count": users.len(),
        "data": users
    })))
}

async fn get_user(
    State(store): State<Store>,
    Path(sub_or_username): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let user = store.get_user(&sub_or_username).await?;
    Ok(Json(json!({
        "success": true,
        "data": user
    })))
}

async fn update_user(
    State(store): State<Store>,
    Path(sub_or_username): Path<String>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = store.update_user(&sub_or_username, payload).await?;
    Ok(Json(json!({
        "success": true,
        "data": user
    })))
}

async fn delete_user(
    State(store): State<Store>,
    Path(sub_or_username): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let deleted = store.delete_user(&sub_or_username).await?;
    if !deleted {
        return Err(AppError::NotFound(format!(
            "User '{sub_or_username}' not found or already deleted"
        )));
    }
    Ok(Json(json!({
        "success": true,
        "message": format!("User '{sub_or_username}' deleted successfully")
    })))
}

async fn assign_role(
    State(store): State<Store>,
    Path(sub_or_username): Path<String>,
    Json(payload): Json<AssignPlatformRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let as_override = payload.as_override.unwrap_or(false);
    let user = store
        .update_user_role(&sub_or_username, payload.role, as_override)
        .await?;
    Ok(Json(json!({
        "success": true,
        "data": user
    })))
}

async fn get_access_context(
    State(store): State<Store>,
    Path(sub_or_username): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let access = store.get_effective_access(&sub_or_username).await?;
    Ok(Json(json!({
        "success": true,
        "data": access
    })))
}

async fn get_user_groups(
    State(store): State<Store>,
    Path(sub_or_username): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let groups = store.list_user_groups(&sub_or_username).await?;
    Ok(Json(json!({
        "success": true,
        "count": groups.len(),
        "data": groups
    })))
}
