/// Federation invite
/// PUT /_matrix/federation/v2/invite/{roomId}/{eventId}
///
/// 他サーバーから invite PDU を受け取り、invitee の room_memberships に記録し、
/// 自サーバーの署名を付与して PDU を返す。
use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, Uri},
    routing::put,
    Json, Router,
};

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/_matrix/federation/v2/invite/:room_id/:event_id",
        put(invite),
    )
}

async fn invite(
    State(state): State<AppState>,
    Path((room_id, _event_id)): Path<(String, String)>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let claims = crate::xmatrix::verify_request(&state, &headers, "PUT", &uri, Some(&body)).await?;

    let mut pdu = body["event"]
        .clone()
        .as_object()
        .cloned()
        .ok_or_else(|| crate::error::AppError::BadRequest("missing event".into()))?;

    let event_type = pdu
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::error::AppError::BadRequest("missing type".into()))?;
    if event_type != "m.room.member" {
        return Err(crate::error::AppError::BadRequest(
            "expected m.room.member event".into(),
        ));
    }

    let membership = pdu
        .get("content")
        .and_then(|c| c["membership"].as_str())
        .unwrap_or("");
    if membership != "invite" {
        return Err(crate::error::AppError::BadRequest(
            "expected membership: invite".into(),
        ));
    }

    let sender = pdu
        .get("sender")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::error::AppError::BadRequest("missing sender".into()))?
        .to_string();

    // state_key が invitee の user_id
    let invitee = pdu
        .get("state_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::error::AppError::BadRequest("missing state_key".into()))?
        .to_string();

    // invitee が自サーバーのユーザーかを確認
    let expected_suffix = format!(":{}", state.server_name);
    if !invitee.ends_with(&expected_suffix) {
        return Err(crate::error::AppError::BadRequest(
            "invitee is not a user on this server".into(),
        ));
    }

    // PDU の送信元署名を検証
    crate::xmatrix::verify_pdu_signatures(
        &state,
        &serde_json::Value::Object(pdu.clone()),
        &claims.origin,
    )
    .await
    .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;

    // room が存在しない場合は作成しない（招待だけ記録）
    // invitee が DB に存在する場合のみ room_memberships に記録する
    // ローカルユーザーでなければ 404 を返す
    let user_exists = db::users::exists(&state.pool, &invitee)
        .await
        .unwrap_or(false);
    if !user_exists {
        return Err(crate::error::AppError::NotFound);
    }

    // ルームが DB に存在しない場合は invite 用に仮登録する
    db::rooms::ensure_placeholder(&state.pool, &room_id)
        .await
        .map_err(crate::error::AppError::Internal)?;

    // room_memberships に invite を記録
    db::rooms::invite(&state.pool, &room_id, &sender, &invitee)
        .await
        .map_err(crate::error::AppError::Internal)?;

    // 自サーバーの署名を PDU に追加
    // signatures フィールドを除いたカノニカル JSON に署名する
    let mut pdu_for_signing = pdu.clone();
    pdu_for_signing.remove("signatures");
    let canonical = crate::signing_key::canonical_json(&serde_json::Value::Object(pdu_for_signing));
    let sig = state.signing_key.sign(canonical.as_bytes());

    let key_id = &state.signing_key.key_id;
    let sigs = pdu
        .entry("signatures")
        .or_insert_with(|| serde_json::json!({}));
    sigs[&*state.server_name][key_id] = serde_json::Value::String(sig);

    Ok(Json(serde_json::json!({
        "event": serde_json::Value::Object(pdu),
    })))
}
