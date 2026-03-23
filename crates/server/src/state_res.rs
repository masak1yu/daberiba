/// Matrix 状態解決（State Resolution Algorithm v2 簡易版）
///
/// 完全な v2 アルゴリズムには PDU の auth_events / prev_events グラフが必要だが、
/// 本実装では実用的なサブセットとして以下のタイブレーカーを提供する:
///
/// 1. origin_server_ts が新しい方を採用
/// 2. 同一タイムスタンプの場合は event_id の辞書順で小さい方を採用（決定論的）
///
/// これは Matrix spec §3.3 「Handling conflicting state」の最終タイブレーカーと同等。
///
/// 実際の状態解決ロジックは `db::events::store_pdu` 内の SQL で実装している。
/// ここでは同じルールを Rust で表現したもの（テスト・将来の拡張用）。
/// 2 つの PDU のうち、状態イベントとして採用すべき event_id を返す。
///
/// `a_ts` / `b_ts` は `origin_server_ts`（ミリ秒）。
/// 戻り値 `true` = a を採用、`false` = b を採用。
#[allow(dead_code)]
pub fn wins(a_id: &str, a_ts: i64, b_id: &str, b_ts: i64) -> bool {
    match a_ts.cmp(&b_ts) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        // タイムスタンプが同一: event_id が辞書順で小さい方（先着）を採用
        std::cmp::Ordering::Equal => a_id < b_id,
    }
}
