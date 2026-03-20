use axum_test::TestServer;
use serde_json::json;

use crate::{router, state::AppState};

async fn test_server() -> TestServer {
    // テスト用インメモリ SQLite は使わず、versions エンドポイントなど DB 不要なものをテスト
    // DB 依存テストは統合テストで別途行う
    let pool = sqlx::MySqlPool::connect_lazy("mysql://matrix:matrix@127.0.0.1:13306/matrix")
        .expect("lazy connect");
    let state = AppState::new(pool);
    let app = router::build(state);
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn versions_returns_supported_versions() {
    let server = test_server().await;
    let res = server.get("/_matrix/client/versions").await;
    res.assert_status_ok();
    let body: serde_json::Value = res.json();
    assert!(body["versions"].as_array().is_some_and(|v| !v.is_empty()));
}

#[tokio::test]
async fn well_known_client_returns_homeserver() {
    let server = test_server().await;
    let res = server.get("/.well-known/matrix/client").await;
    res.assert_status_ok();
    let body: serde_json::Value = res.json();
    assert!(body["m.homeserver"]["base_url"].is_string());
}

#[tokio::test]
async fn capabilities_returns_room_versions() {
    let server = test_server().await;
    let res = server.get("/_matrix/client/v3/capabilities").await;
    res.assert_status_ok();
    let body: serde_json::Value = res.json();
    assert!(body["capabilities"]["m.room_versions"].is_object());
}

#[tokio::test]
async fn login_flows_returns_password_flow() {
    let server = test_server().await;
    let res = server.get("/_matrix/client/v3/login").await;
    res.assert_status_ok();
    let body: serde_json::Value = res.json();
    let flows = body["flows"].as_array().unwrap();
    assert!(flows.iter().any(|f| f["type"] == "m.login.password"));
}

#[tokio::test]
async fn register_rejects_short_password() {
    let server = test_server().await;
    let res = server
        .post("/_matrix/client/v3/register")
        .json(&json!({ "username": "testuser", "password": "short" }))
        .await;
    res.assert_status_bad_request();
    let body: serde_json::Value = res.json();
    assert_eq!(body["errcode"], "M_BAD_JSON");
}

#[tokio::test]
async fn register_rejects_invalid_username() {
    let server = test_server().await;
    let res = server
        .post("/_matrix/client/v3/register")
        .json(&json!({ "username": "invalid user!", "password": "validpassword" }))
        .await;
    res.assert_status_bad_request();
}

#[tokio::test]
async fn protected_endpoint_requires_auth() {
    let server = test_server().await;
    let res = server.get("/_matrix/client/v3/account/whoami").await;
    res.assert_status_unauthorized();
}

#[tokio::test]
async fn sync_requires_auth() {
    let server = test_server().await;
    let res = server.get("/_matrix/client/v3/sync").await;
    res.assert_status_unauthorized();
}
