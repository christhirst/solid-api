use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::error::AppError;
use crate::models::{AssignGroupMemberRequest, CreateGroupRequest};
use crate::store::Store;

pub fn routes() -> Router<Store> {
    Router::new()
        .route("/", post(create_group).get(list_groups))
        .route("/{name}", get(get_group).delete(delete_group))
        .route("/{name}/members", get(list_members).post(assign_member))
        .route("/{name}/members/{username}", delete(remove_member))
}

async fn create_group(
    State(store): State<Store>,
    Json(payload): Json<CreateGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let group = store.create_group(payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "data": group
        })),
    ))
}

async fn list_groups(State(store): State<Store>) -> Result<impl IntoResponse, AppError> {
    let groups = store.list_groups().await?;
    Ok(Json(json!({
        "success": true,
        "count": groups.len(),
        "data": groups
    })))
}

async fn get_group(
    State(store): State<Store>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let group = store.get_group(&name).await?;
    Ok(Json(json!({
        "success": true,
        "data": group
    })))
}

async fn delete_group(
    State(store): State<Store>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let deleted = store.delete_group(&name).await?;
    if !deleted {
        return Err(AppError::NotFound(format!(
            "Group '{name}' not found or already deleted"
        )));
    }
    Ok(Json(json!({
        "success": true,
        "message": format!("Group '{name}' deleted successfully")
    })))
}

async fn list_members(
    State(store): State<Store>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let members = store.list_group_members(&name).await?;
    Ok(Json(json!({
        "success": true,
        "count": members.len(),
        "data": members
    })))
}

async fn assign_member(
    State(store): State<Store>,
    Path(name): Path<String>,
    Json(payload): Json<AssignGroupMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let member = store.assign_group_role(&name, payload).await?;
    Ok(Json(json!({
        "success": true,
        "data": member
    })))
}

async fn remove_member(
    State(store): State<Store>,
    Path((name, username)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let removed = store.remove_group_member(&name, &username).await?;
    if !removed {
        return Err(AppError::NotFound(format!(
            "Member '{username}' not found in group '{name}'"
        )));
    }
    Ok(Json(json!({
        "success": true,
        "message": format!("Member '{username}' removed from group '{name}'")
    })))
}
