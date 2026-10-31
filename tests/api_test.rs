use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use solid_api::api::build_app;
use solid_api::store::Store;
use tower::ServiceExt;

#[tokio::test]
async fn test_rest_api_lifecycle() {
    let store = Store::new_in_memory().await.expect("in-memory store");
    let app = build_app(store);

    // 1. GET /health
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. POST /api/v1/schema/init
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/schema/init")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. POST /api/v1/users (provision user)
    let body = json!({
        "sub": "sub_eve",
        "username": "Eve",
        "name": "Eve Operator",
        "email": "eve@example.com",
        "role": "creator",
        "namespace": "eve-space"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/users")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let res_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(res_json["success"], true);
    assert_eq!(res_json["data"]["username"], "eve");
    assert_eq!(res_json["data"]["idpRole"], "creator");

    // 4. GET /api/v1/users/eve
    let req = Request::builder()
        .uri("/api/v1/users/eve")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. POST /api/v1/users/eve/role (assign role)
    let role_body = json!({
        "role": "admin",
        "as_override": false
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/users/eve/role")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&role_body).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let res_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(res_json["data"]["idpRole"], "admin");

    // 6. POST /api/v1/groups (create group)
    let grp_body = json!({
        "name": "devops",
        "owner": "sub_eve",
        "description": "DevOps Space"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/groups")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&grp_body).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 7. GET /api/v1/groups
    let req = Request::builder()
        .uri("/api/v1/groups")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let res_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(res_json["count"], 1);

    // 8. GET /api/v1/users/eve/access (effective access)
    let req = Request::builder()
        .uri("/api/v1/users/eve/access")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let res_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(res_json["data"]["is_admin"], true);
    assert_eq!(
        res_json["data"]["capabilities"]["can_manage_database"],
        true
    );

    // 9. DELETE /api/v1/users/eve
    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/users/eve")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
