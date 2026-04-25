use crate::{
    error::{ApiResult, AppError},
    middleware::auth::AuthUser,
    state::AppState,
};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use serde::Deserialize;
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        // 旧 v3 メディア API
        .route(
            "/_matrix/media/v3/upload",
            post(upload).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route(
            "/_matrix/media/v3/download/:server_name/:media_id",
            get(download),
        )
        .route(
            "/_matrix/media/v3/download/:server_name/:media_id/:filename",
            get(download_with_filename),
        )
        // サムネイル（v3・MSC3916 v1 共通: フル画像を返す）
        .route(
            "/_matrix/media/v3/thumbnail/:server_name/:media_id",
            get(thumbnail),
        )
        // MSC3916 認証済みメディア API（v1）
        .route(
            "/_matrix/client/v1/media/upload",
            post(upload).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route(
            "/_matrix/client/v1/media/download/:server_name/:media_id",
            get(download),
        )
        .route(
            "/_matrix/client/v1/media/download/:server_name/:media_id/:filename",
            get(download_with_filename),
        )
        .route(
            "/_matrix/client/v1/media/thumbnail/:server_name/:media_id",
            get(thumbnail),
        )
}

#[derive(Deserialize)]
struct UploadQuery {
    filename: Option<String>,
    /// アップロード先ルーム ID（指定するとダウンロードをルームメンバーに制限）
    room_id: Option<String>,
}

async fn upload(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<axum::Json<serde_json::Value>> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let media_id = Uuid::new_v4().to_string().replace('-', "");
    let file_size = body.len() as i64;

    state.media.store(&media_id, body).await?;

    db::media::insert(
        &state.pool,
        &media_id,
        &state.server_name,
        &user.user_id,
        &content_type,
        query.filename.as_deref(),
        file_size,
        query.room_id.as_deref(),
    )
    .await?;

    let mxc_uri = format!("mxc://{}/{}", state.server_name, media_id);
    Ok(axum::Json(serde_json::json!({ "content_uri": mxc_uri })))
}

async fn download(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((server_name, media_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    serve_media(&state, &server_name, &media_id, &user.user_id).await
}

async fn download_with_filename(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((server_name, media_id, _filename)): Path<(String, String, String)>,
) -> Result<Response, AppError> {
    serve_media(&state, &server_name, &media_id, &user.user_id).await
}

/// サムネイルクエリパラメータ。
#[derive(Deserialize)]
struct ThumbnailQuery {
    width: Option<u32>,
    height: Option<u32>,
    /// "scale"（アスペクト比維持）または "crop"（クロップ）。デフォルト "scale"。
    method: Option<String>,
}

async fn thumbnail(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response, AppError> {
    let record = db::media::get(&state.pool, &server_name, &media_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let accessible = db::media::is_accessible_by(&state.pool, &record, &user.user_id)
        .await
        .map_err(AppError::Internal)?;
    if !accessible {
        return Err(AppError::Forbidden);
    }

    let data = state
        .media
        .fetch(&media_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // width/height 未指定、または画像でない場合はフル画像をそのまま返す
    let (Some(w), Some(h)) = (query.width, query.height) else {
        return build_response(&record.content_type, data);
    };

    // content_type が image/* でない場合はそのまま返す
    if !record.content_type.starts_with("image/") {
        return build_response(&record.content_type, data);
    }

    // image クレートでリサイズ
    let img = image::load_from_memory(&data).map_err(|e| AppError::Internal(e.into()))?;

    let resized = if query.method.as_deref() == Some("crop") {
        // crop: アスペクト比を無視してクロップ
        img.resize_to_fill(w, h, image::imageops::FilterType::Lanczos3)
    } else {
        // scale（デフォルト）: アスペクト比を維持して縮小
        img.resize(w, h, image::imageops::FilterType::Lanczos3)
    };

    // JPEG でエンコードして返す
    let mut buf = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| AppError::Internal(e.into()))?;

    build_response("image/jpeg", bytes::Bytes::from(buf.into_inner()))
}

fn build_response(content_type: &str, data: bytes::Bytes) -> Result<Response, AppError> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, data.len())
        .body(Body::from(data))
        .map_err(|e| AppError::Internal(e.into()))
}

async fn serve_media(
    state: &AppState,
    server_name: &str,
    media_id: &str,
    user_id: &str,
) -> Result<Response, AppError> {
    let record = db::media::get(&state.pool, server_name, media_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // アクセス制御: room_id が設定されていればルームメンバーのみ
    let accessible = db::media::is_accessible_by(&state.pool, &record, user_id)
        .await
        .map_err(AppError::Internal)?;
    if !accessible {
        return Err(AppError::Forbidden);
    }

    let data = state
        .media
        .fetch(media_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let response = build_response(&record.content_type, data)?;

    Ok(response)
}
