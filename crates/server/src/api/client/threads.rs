use crate::{error::ApiResult, middleware::auth::AuthUser, state::AppState};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new().route("/_matrix/client/v1/rooms/:roomId/threads", get(get_threads))
}

#[derive(Deserialize)]
struct ThreadsQuery {
    /// ページネーショントークン（前ページ末尾の stream_ordering の文字列）
    from: Option<String>,
    /// 取得件数（デフォルト 20、最大 50）
    limit: Option<u32>,
    /// "all"（デフォルト）または "participated"（自分が参加したスレッドのみ）
    include: Option<String>,
}

#[derive(Serialize)]
struct ThreadsResponse {
    chunk: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_batch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prev_batch: Option<String>,
}

/// GET /_matrix/client/v1/rooms/:roomId/threads
/// ルーム内のスレッド一覧を最新活動順で返す。
async fn get_threads(
    State(state): State<AppState>,
    axum::Extension(user): axum::Extension<AuthUser>,
    Path(room_id): Path<String>,
    Query(query): Query<ThreadsQuery>,
) -> ApiResult<Json<ThreadsResponse>> {
    use sqlx::Row;

    let limit = query.limit.unwrap_or(20).min(50) as i64;
    let before_ordering: Option<i64> = query.from.as_deref().and_then(|s| s.parse().ok());
    let include_participated = query.include.as_deref() == Some("participated");

    // スレッドルートと最新活動の stream_ordering を集計する。
    // m.thread リレーションのうち、ルーム内のものを集め、
    // relates_to_event_id（スレッドルート）ごとに最新の stream_ordering を取得。
    let cursor_clause = if before_ordering.is_some() {
        "AND latest_activity < ?"
    } else {
        ""
    };

    let participated_clause = if include_participated {
        "AND EXISTS (
            SELECT 1 FROM events ep
            INNER JOIN event_relations erp ON erp.event_id = ep.event_id
            WHERE erp.relates_to_event_id = thread_roots.root_event_id
              AND erp.rel_type = 'm.thread'
              AND ep.sender = ?
         )"
    } else {
        ""
    };

    let sql = format!(
        r#"SELECT thread_roots.root_event_id,
                  thread_roots.latest_activity,
                  thread_roots.reply_count,
                  latest_ev.event_id AS latest_event_id
           FROM (
               SELECT er.relates_to_event_id AS root_event_id,
                      MAX(e.stream_ordering)  AS latest_activity,
                      COUNT(*)                AS reply_count
               FROM event_relations er
               INNER JOIN events e ON e.event_id = er.event_id
               WHERE e.room_id = ?
                 AND er.rel_type = 'm.thread'
               GROUP BY er.relates_to_event_id
           ) AS thread_roots
           INNER JOIN events latest_ev
             ON latest_ev.stream_ordering = thread_roots.latest_activity
           WHERE 1=1
             {}
             {}
           ORDER BY thread_roots.latest_activity DESC
           LIMIT ?"#,
        cursor_clause, participated_clause,
    );

    let mut q = sqlx::query(&sql).bind(&room_id);
    if let Some(ord) = before_ordering {
        q = q.bind(ord);
    }
    if include_participated {
        q = q.bind(&user.user_id);
    }
    // limit+1 件取得してページ継続を判定
    let rows = q.bind(limit + 1).fetch_all(&state.pool).await?;

    let has_more = rows.len() as i64 == limit + 1;
    let rows = if has_more {
        &rows[..rows.len() - 1]
    } else {
        &rows[..]
    };

    // スレッドルートイベントを取得して unsigned.m.relations.m.thread を付与する
    let mut chunk = Vec::with_capacity(rows.len());
    for row in rows {
        let root_event_id: String = row.get("root_event_id");
        let reply_count: i64 = row.get("reply_count");
        let latest_event_id: String = row.get("latest_event_id");

        // ルートイベント本体（unsigned.m.relations は get_by_id が付与済み）
        let mut event = db::events::get_by_id(&state.pool, &root_event_id)
            .await
            .unwrap_or_default()
            .unwrap_or_default();

        // スレッド内最新イベントを取得
        let latest_event = db::events::get_by_id(&state.pool, &latest_event_id)
            .await
            .unwrap_or_default()
            .unwrap_or_default();

        // m.thread 集計を unsigned.m.relations に追加
        let thread_summary = serde_json::json!({
            "latest_event": latest_event,
            "count": reply_count,
            "current_user_participated": include_participated,
        });

        if let Some(unsigned) = event.get_mut("unsigned").and_then(|u| u.as_object_mut()) {
            if let Some(relations) = unsigned
                .get_mut("m.relations")
                .and_then(|r| r.as_object_mut())
            {
                relations.insert("m.thread".to_string(), thread_summary);
            } else {
                unsigned.insert(
                    "m.relations".to_string(),
                    serde_json::json!({ "m.thread": thread_summary }),
                );
            }
        } else {
            event["unsigned"] =
                serde_json::json!({ "m.relations": { "m.thread": thread_summary } });
        }

        chunk.push(event);
    }

    let next_batch = if has_more {
        rows.last()
            .map(|r| r.get::<i64, _>("latest_activity").to_string())
    } else {
        None
    };

    let prev_batch = query.from;

    Ok(Json(ThreadsResponse {
        chunk,
        next_batch,
        prev_batch,
    }))
}
