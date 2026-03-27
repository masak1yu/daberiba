# Handover — v0.47.0 → v0.48.0

## v0.47.0 でやったこと

- **非メンバーのイベント送信を 403 で拒否** (`server/api/client/events.rs`):
  - `PUT /rooms/{roomId}/send/{eventType}/{txnId}` で `db::rooms::get_membership()` を呼び出し、`membership != 'join'` の場合 403 Forbidden を返すよう変更。

- **state イベント送信のパワーレベル確認** (`server/api/client/events.rs`):
  - `check_state_event_power()` ヘルパー新設。`m.room.power_levels` 状態イベントを取得し、ユーザーの power level と `events[event_type]`（なければ `state_default: 50`）を比較。
  - `PUT /rooms/{roomId}/state/{type}` および `PUT /rooms/{roomId}/state/{type}/{key}` の両ハンドラで呼び出す。
  - メンバーシップ確認も内包（非 join ユーザーは 403）。

- **メディアサムネイルリサイズ** (`server/api/media.rs`, `Cargo.toml`):
  - `image = "0.25"` クレートを workspace に追加（jpeg/png/webp/gif のみ有効化）。
  - `GET /thumbnail/{serverName}/{mediaId}?width=W&height=H&method=scale|crop` で実際にリサイズして JPEG で返すよう変更。
  - `?width=`/`?height=` が未指定の場合、または `image/*` 以外の場合はフル画像をそのまま返す。
  - `method=crop` は `resize_to_fill`（Lanczos3）、`scale`（デフォルト）は `resize`。

## v0.46.0 でやったこと

- **SSO/OIDC ログインフロー** (`schema/schema.sql`, `db/sso.rs`, `server/sso.rs`, `server/api/client/auth.rs`):
  - `sso_states` テーブル新設（state トークン → redirect_url、5 分有効、使い捨て）。
  - `sso_accounts` テーブル新設（OIDC sub → Matrix user_id マッピング）。
  - `db::sso::create_state()` / `consume_state()` / `find_user_by_sub()` / `link_account()` 新設。
  - `server::sso::SsoConfig` 構造体 — 起動時に `OIDC_ISSUER` の discovery エンドポイントを叩いて auth/token/userinfo URL を取得。`OIDC_ISSUER` が未設定なら SSO 無効。
  - `AppState.sso: Option<Arc<SsoConfig>>` を追加。
  - `GET /_matrix/client/v3/login` — SSO 有効時は `m.login.sso` フローと `identity_providers` を含めるよう変更。
  - `GET /_matrix/client/v3/login/sso/redirect?redirectUrl=<url>` — state を生成して OIDC 認可 URL へリダイレクト（HTTP 307）。
  - `GET /_matrix/client/v3/login/sso/redirect/{idpId}` — 同上（現状は idpId 無視、1 プロバイダーのみ）。
  - `GET /_matrix/client/v3/login/sso/callback?code=...&state=...` — code を token に交換 → userinfo 取得 → sso_accounts でユーザーを解決（初回は自動登録・display_name セット）→ login_token を発行 → `redirectUrl?loginToken=<token>` へリダイレクト。
  - `derive_localpart()` / `sanitize_localpart()` / `make_unique_localpart()` ヘルパー追加。
  - `urlencoding` クレートを workspace に追加。

## v0.45.0 でやったこと

- **ユーザーディレクトリ検索** (`db/users.rs`, `server/api/client/user_directory.rs`):
  - `db::users::search_directory(pool, term, limit)` 新設。`users` テーブルを `profiles` と LEFT JOIN し、`user_id LIKE` または `display_name LIKE` で部分一致検索。非アクティブユーザーは除外。
  - `POST /_matrix/client/v3/user_directory/search` を新設。`{ search_term, limit }` を受け取り `{ results: [{user_id, display_name?, avatar_url?}], limited }` を返す。

- **メディアサムネイル** (`server/api/media.rs`):
  - `GET /_matrix/media/v3/thumbnail/{serverName}/{mediaId}` を新設。`?width=`/`?height=`/`?method=` を受け付けるが、現状はリサイズせずフル画像を返す。
  - `GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}` も同様（MSC3916）。

- **MSC3916 認証済みメディアエンドポイント** (`server/api/media.rs`):
  - `POST /_matrix/client/v1/media/upload` — 既存 upload ハンドラを流用。
  - `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}[/{filename}]` — 既存 download ハンドラを流用。
  - `GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}` — 既存 thumbnail ハンドラを流用。

- **サードパーティプロトコルスタブ** (`server/api/client/thirdparty.rs`):
  - `GET /_matrix/client/v3/thirdparty/protocols` を新設。ブリッジ未設定のため空オブジェクト `{}` を返す。

## v0.44.0 でやったこと

- **グローバルイベントストリーム** (`db/events.rs`, `server/api/client/global_events.rs`):
  - `db::events::get_global_events_since(pool, user_id, since_ordering, room_id_filter, limit)` 新設。ユーザーが参加しているルームのイベントを stream_ordering 昇順で返す。
  - `GET /_matrix/client/v3/events` レガシーエンドポイントを新設。`?from=<token>` でページネーション、`?room_id=` で特定ルーム絞り込み、`?timeout=` を受け付けるが現状は即時返却。

- **`/publicRooms` cross-server プロキシ** (`server/api/client/public_rooms.rs`):
  - `GET /publicRooms?server=<serverName>` および POST body `server` フィールドに対応。
  - 指定サーバーの `/_matrix/federation/v1/publicRooms` に `state.http` でプロキシし、レスポンスをそのまま返す。
  - `percent_encode()` ヘルパーで filter クエリパラメータを URL エンコード。

## v0.43.0 でやったこと

- **txn_id 冪等性** (`schema/schema.sql`, `db/sent_transactions.rs`, `server/api/client/events.rs`):
  - `sent_transactions` テーブル新設 (`user_id`, `device_id`, `txn_id`, `event_id`)。PK は `(user_id, device_id, txn_id)`。
  - `db::sent_transactions::get_event_id()` — 既存 txn_id があれば event_id を返す。
  - `db::sent_transactions::record()` — 送信後に INSERT IGNORE で記録。
  - `PUT /rooms/{roomId}/send/{eventType}/{txnId}` — 送信前に重複チェック。同一 (device, txn_id) は保存済み event_id をそのまま返す。送信後に記録。

- **`/rooms/{roomId}/initialSync`** (`server/api/client/initial_sync.rs`):
  - `GET /_matrix/client/v3/rooms/{roomId}/initialSync` レガシーエンドポイントを新設。
  - `membership`, `state`（現在のスナップショット）, `messages.chunk`（最新50件・後方向き）, `start`/`end` トークン, `receipts` を返す。
  - 既存の `db::room_state::get_all()`, `db::events::get_messages()`, `db::receipts::get_for_room()` を活用。

- **room versions 9/11 追加** (`server/api/client/capabilities.rs`):
  - `/capabilities` の `m.room_versions.available` に `"9": "stable"` と `"11": "stable"` を追加。

## v0.42.0 でやったこと

- **ノック（MSC2403）** (`db/rooms.rs`, `server/api/client/rooms.rs`, `db/sync.rs`):
  - `db::rooms::knock()` 新設。`room_memberships.membership = 'knock'` に upsert。
  - `db::rooms::knock_rooms()` 新設。ユーザーが knock 中のルーム ID 一覧を返す。
  - `POST /_matrix/client/v3/rooms/{roomId}/knock` — join_rules が `knock` or `knock_restricted` の場合のみ許可。ban 済みユーザーと既参加ユーザーはブロック。m.room.member イベント（membership=knock）を生成。
  - `POST /_matrix/client/v3/knock/{roomIdOrAlias}` — エイリアス（#...）の場合は `db::room_aliases::resolve()` で room_id に変換してから knock。
  - `/sync` の `rooms.knock` セクションを追加。knock 中ルームの stripped state（name/avatar/join_rules/canonical_alias/encryption）を `knock_state.events` で返す。

- **`GET /_matrix/client/v3/register/available`** (`server/api/client/auth.rs`):
  - `?username=<localpart>` でユーザー名利用可否を確認。`@localpart:SERVER_NAME` が存在しなければ `{ available: true }` を返す。存在する場合は 400 M_USER_IN_USE。
  - 既存の `db::users::exists()` を利用。

## v0.41.0 でやったこと

- **OpenID Connect トークン発行** (`schema/schema.sql`, `db/openid_tokens.rs`, `server/api/client/openid.rs`, `server/api/federation/openid.rs`):
  - `openid_tokens` テーブル新設 (`token`, `user_id`, `expires_at`)。有効期限 3600 秒。
  - `db::openid_tokens::create()` — UUID トークンを発行して保存。
  - `db::openid_tokens::verify()` — トークンの有効性確認と user_id 取得。期限切れは None。
  - `db::openid_tokens::purge_expired()` — 期限切れトークンの削除（クリーンアップ用）。
  - `POST /_matrix/client/v3/user/{userId}/openid/request_token` — 自分専用に `{ access_token, token_type: "Bearer", matrix_server_name, expires_in: 3600 }` を返す。
  - `GET /_matrix/federation/v1/openid/userinfo?access_token=<token>` — 外部サービスがトークンを検証して `{ sub: "@user:server" }` を取得。

- **ルームサマリー** (`db/rooms.rs`, `server/api/client/room_summary.rs`):
  - `db::rooms::get_membership()` 新設。ユーザーのルームに対する membership を取得。
  - `GET /_matrix/client/v1/rooms/{roomId}/summary` (MSC3266) を新設。
  - `m.room.name`, `m.room.topic`, `m.room.avatar`, `m.room.join_rules`, `m.room.canonical_alias`, `m.room.create`, `m.room.encryption`, `m.room.guest_access` を `tokio::join!` で並列取得。
  - `num_joined_members`, `join_rule`, `world_readable`, `guest_can_join`, `membership` (リクエストユーザーの) を返す。フィールドが存在する場合のみレスポンスに含める。

## v0.40.0 でやったこと

- **クロスサイニングキー** (`schema/schema.sql`, `db/keys.rs`, `server/api/client/keys.rs`):
  - `cross_signing_keys` テーブル新設 (`user_id`, `key_type`, `key_json`)。
  - `key_signatures` テーブル新設 (`signer_user_id`, `target_user_id`, `key_id`, `signature_json`)。
  - `db::keys::upload_cross_signing_keys()` / `get_cross_signing_keys()` 新設。
  - `db::keys::upload_key_signature()` / `get_key_signatures()` 新設。
  - `POST /_matrix/client/v3/keys/device_signing/upload` — master/self_signing/user_signing キーをアップロード。
  - `POST /_matrix/client/v3/keys/signatures/upload` — 署名オブジェクトをアップロード（失敗時は `failures` マップで返す）。
  - `POST /keys/query` レスポンスに `master_keys`, `self_signing_keys`, `user_signing_keys` を追加。

- **`GET /keys/changes`** (`server/api/client/keys.rs`):
  - `GET /_matrix/client/v3/keys/changes?from=<token>&to=<token>` を新設。
  - from トークン（sync トークン形式 `{ord}_{td}_{ms}`）から `since_ms` と `since_stream` を抽出し、既存の `get_changed_users` / `get_left_users` を呼び出す。
  - `{ changed: [...], left: [...] }` を返す。

- **`/sync` presence デルタ** (`db/presence.rs`, `server/api/client/sync.rs`):
  - `db::presence::get_changed_since(pool, user_ids, since_ms)` 新設。IN 句で対象ユーザーを絞り、`last_active_ts > since_ms` で差分取得。
  - `/sync` の presence 収集: `account_data_since_ms` がある（2 回目以降 sync）場合は `get_changed_since` で差分のみ返す。初回 sync は従来通り全員分。

## v0.39.0 でやったこと

- **`/capabilities` 拡充** (`server/api/client/capabilities.rs`):
  - `m.set_displayname: { enabled: true }` — 表示名変更可能フラグを追加。
  - `m.set_avatar_url: { enabled: true }` — アバター URL 変更可能フラグを追加。
  - `m.3pid_changes: { enabled: false }` — 3pid 変更フラグ（現状無効）。
  - `m.get_login_token: { enabled: false }` — ログイントークン取得フラグ（エンドポイントは実装済みだが capability フラグは無効）。

- **`/publicRooms` 改善** (`server/api/client/public_rooms.rs`, `db/rooms.rs`):
  - `GET` に `?limit=`（デフォルト 30、最大 500）、`?since=`（offset ベースのページネーショントークン）、`?filter=`（name/topic の部分一致）パラメータを追加。
  - `POST /_matrix/client/v3/publicRooms` を新設。ボディ `{ "limit", "since", "filter": { "generic_search_term" } }` に対応。
  - レスポンスに `next_batch` / `prev_batch` トークンを追加（次ページが存在する場合のみ）。`total_room_count_estimate` を実際のフィルタ後総件数に変更。
  - `db::rooms::get_public_rooms()` シグネチャを `(pool, filter, limit, offset) -> (Vec<PublicRoom>, u64)` に変更。フィルタ時は `r.name LIKE ? OR r.topic LIKE ?`、ページネーションは LIMIT/OFFSET。

- **`/rooms/{roomId}/upgrade` 改善** (`server/api/client/rooms.rs`):
  - 旧ルームの `m.room.name`、`m.room.topic`、`m.room.avatar` を `db::room_state::get_event()` で取得し、新ルームにコピー。
  - 各状態イベントが存在する場合のみコピー（`None` の場合はスキップ）。

## v0.38.0 でやったこと

- **`POST /rooms/{roomId}/report/{eventId}`** (`schema/schema.sql`, `db/reports.rs`, `server/api/client/room_state.rs`):
  - `event_reports` テーブル新設（room_id, event_id, reporter user_id, score, reason, created_at）。
  - `db::reports::create()` — 報告を記録。`db::reports::list_all()` — 管理者向け全報告一覧。
  - `POST /_matrix/client/v3/rooms/{roomId}/report/{eventId}` — `{ "score": -100, "reason": "spam" }` で報告。Matrix spec 準拠。
  - `GET /_synapse/admin/v1/event_reports` — 管理者向け報告一覧を追加（`admin.rs`）。

- **`/sync` の `rooms.invite` 改善** (`db/sync.rs`):
  - invite_state に含めるストリップドステートを拡充: `m.room.create`, `m.room.join_rules`, `m.room.name`, `m.room.avatar`, `m.room.canonical_alias`, `m.room.encryption` を追加。
  - E2EE ルームでの招待時にクライアントが暗号化状態を判定できるようになった。

- **プッシュルール デフォルトセット拡充** (`server/api/client/events.rs`):
  - override に追加: `.m.rule.invite_for_me`（招待通知）, `.m.rule.member_event`（メンバーイベント抑制）, `.m.rule.tombstone`（廃止部屋通知）, `.m.rule.reaction`（リアクション抑制）, `.m.rule.room.server_acl`（ACL 抑制）。
  - underride に追加: `.m.rule.call`（通話招待）, `.m.rule.encrypted_room_one_to_one`（1対1暗号化）, `.m.rule.room_one_to_one`（1対1メッセージ）。

## v0.37.0 でやったこと

- **管理者昇格 API** (`db/users.rs`, `server/api/client/admin.rs`):
  - `db::users::set_admin()` 新設。`admin` フラグを ON/OFF する。
  - `PUT /_synapse/admin/v1/users/{userId}/admin` — `{ "admin": true/false }` でフラグを切り替え。管理者専用。
  - 対象ユーザーが存在しない場合は 404 を返す。

- **`/admin/media` 管理** (`db/media.rs`, `server/src/media_store.rs`, `server/api/client/admin.rs`):
  - `db::media::list_all()` 新設（非マクロ `sqlx::query_as`）。全メディアレコードを返す。
  - `db::media::delete()` 新設。DB レコードを削除し、削除件数を返す。
  - `MediaStore` トレイトに `async fn delete(&self, media_id: &str) -> Result<()>` を追加。`LocalStore` / `S3Store` に実装。
  - `GET /_synapse/admin/v1/media` — メディア一覧（?from=&limit= ページネーション）。管理者専用。
  - `DELETE /_synapse/admin/v1/media/{serverName}/{mediaId}` — DB レコード削除 + ストレージファイル削除。管理者専用。

- **`/sync` の `device_lists` 改善** (`db/keys.rs`, `server/api/client/sync.rs`):
  - `db::keys::get_changed_users()` 新設。共有ルームにいるユーザーのうち、`since_ms` 以降に `device_keys.updated_at` が更新されたユーザーを返す（初回 sync は全共有ルームメンバー）。
  - `db::keys::get_left_users()` 新設。`since_stream` 以降に leave/ban イベントが発生し、かつ現在共有ルームがないユーザーを返す。
  - `/sync` レスポンスに `device_lists: { changed: [...], left: [...] }` を追加。

## v0.36.0 でやったこと

- **`/rooms/{roomId}/context` の `state` フィールド** (`db/room_state.rs`, `db/events.rs`, `server/api/client/events.rs`):
  - `db::room_state::get_state_at()` 新設。指定 stream_ordering 以前の各 (event_type, state_key) ペアの最新イベントを correlated subquery で取得。
  - `EventContextResult` に `state: Vec<serde_json::Value>` フィールドを追加。
  - `/context` ハンドラで `state: []` の代わりに実際のスナップショットを返すよう変更。

- **`/sendToDevice` の `device_id: *` 展開** (`db/to_device.rs`, `server/api/client/to_device.rs`, `server/api/client/sync.rs`):
  - `device_id = "*"` の場合、送信時に `db::devices::list()` で受信者の全デバイスを取得し、デバイスごとに個別レコードを挿入。
  - `db::to_device::get_pending()` のシグネチャを `(pool, user_id, device_id)` に変更し、自デバイス宛てのメッセージのみ取得するよう修正。
  - `/sync` で `user.device_id` を渡すよう更新。

- **`/admin/*` 管理 API** (`schema/schema.sql`, `db/users.rs`, `db/rooms.rs`, `server/api/client/admin.rs`):
  - `users` テーブルに `admin TINYINT(1) DEFAULT 0` カラムを追加。
  - `db::users::is_admin()` — 管理者フラグ確認。
  - `db::users::list_all()` — 全ユーザー一覧（display_name, avatar_url, deactivated, admin, creation_ts）。
  - `db::users::admin_deactivate()` — `deactivated=1` + `password_hash=''` + 全アクセストークン削除。
  - `db::rooms::list_all()` — 全ルーム一覧（name, creator, joined_members, creation_ts）。
  - `GET /_matrix/client/v3/admin/whois/{userId}` — デバイス別セッション情報（自分か管理者のみ）。
  - `GET /_synapse/admin/v1/users` — ユーザー一覧（管理者専用、?from=&limit= ページネーション）。
  - `GET /_synapse/admin/v1/users/{userId}` — ユーザー詳細（管理者専用）。
  - `POST /_synapse/admin/v1/deactivate/{userId}` — ユーザー無効化（管理者専用）。
  - `GET /_synapse/admin/v1/rooms` — ルーム一覧（管理者専用、?from=&limit= ページネーション）。

## v0.35.0 でやったこと

- **`/rooms/{roomId}/members?at=<token>`** (`server/api/client/room_state.rs`, `db/rooms.rs`):
  - `?at=` に prev_batch / next_batch トークン（"s{stream_ordering}" 形式）を指定すると、events テーブルから m.room.member イベントを再構築して時点スナップショットを返す。
  - correlated subquery で各 state_key の最大 stream_ordering を取得し、membership / not_membership フィルタと組み合わせ可能。
  - `db::rooms::get_members_at()` 新設。

- **`/rooms/{roomId}/messages?lazy_load_members=true`** (`server/api/client/events.rs`, `db/rooms.rs`):
  - `?lazy_load_members=true` を指定すると、レスポンスの `state` フィールドに chunk 内の sender に対応する m.room.member イベントを含める。
  - `db::rooms::get_member_events_for_users()` 新設（IN 句で一括取得）。
  - `MessagesResponse` に `state: Vec<serde_json::Value>` フィールドを追加（空の場合はシリアライズ省略）。

- **`lazy_load_members` フィルタ** (`server/filter.rs`, `server/api/client/sync.rs`):
  - フィルター JSON の `room.state.lazy_load_members: true` を `FilterDef` に追加。
  - sync の state イベントを後処理でフィルタリング — timeline に現れた sender の `m.room.member` のみ残し、非メンバーイベントはそのまま返す。

- **`/rooms/{roomId}/hierarchy?max_depth=<n>`** (`server/api/client/hierarchy.rs`):
  - `?max_depth=<n>`（デフォルト 1、最大 5）を追加。BFS で n 層まで再帰的にスペース階層を展開。
  - `visited` セットで循環を防ぐ。子の children_state にはグランドチルドレンのイベントを付与。
  - ページネーションはルート直下の直接チルドレンに適用（深い階層は再帰で一括展開）。

- **release.yml の修正** (`.github/workflows/release.yml`):
  - workspace 版一元化後に `version.workspace = true` がタグ名になるバグを修正。
  - バージョン読み取り・バンプ対象を `crates/server/Cargo.toml` → ルート `Cargo.toml` に変更。

## v0.34.0 でやったこと

- **`m.login.token` フロー** (`server/api/client/auth.rs`, `db/login_tokens.rs`, `schema.sql`):
  - `POST /_matrix/client/v1/login/get_token` — 認証済みセッションから 120 秒有効・シングルユースのログイントークンを発行。
  - `POST /_matrix/client/v3/login` に `type: "m.login.token"` を追加。トークン消費後に新デバイス + アクセストークンを発行。
  - `login_tokens` テーブル新設（`token`, `user_id`, `expires_at`, `used`）。
  - `GET /login` の `flows` に `m.login.token` を追加。

- **`/account/3pid` 管理** (`server/api/client/threepids.rs`, `db/threepids.rs`, `schema.sql`):
  - `GET /_matrix/client/v3/account/3pids` — ユーザーに紐づく 3pid 一覧を返す。
  - `POST /_matrix/client/v3/account/3pid/add` — 3pid（email/msisdn）を直接登録（identity server バリデーションなし）。
  - `POST /_matrix/client/v3/account/3pid/delete` — 3pid を削除。レスポンスは `id_server_unbind_result: "no-support"`。
  - `user_threepids` テーブル新設（`medium + address` PK, `user_id`, `validated_at`, `added_at`）。

- **`GET /rooms/{roomId}/hierarchy`** (`server/api/client/hierarchy.rs`):
  - `/_matrix/client/v1/rooms/{roomId}/hierarchy` — スペース階層取得（MSC2946）。
  - `m.space.child` 状態イベントを走査してチルドレンを 1 層分列挙。
  - `?suggested_only=true` で `content.suggested=true` の子のみ返す。
  - `?from=<room_id>&limit=<n>` によるカーソルページネーション。
  - レスポンスに `room_type`, `world_readable`, `guest_can_join`, `join_rule`, `children_state` を含む。

- **`/rooms/{roomId}/members` フィルタ** (`server/api/client/room_state.rs`, `db/rooms.rs`):
  - `?membership=<value>` — 特定 membership ステートのメンバーのみ返す（join / leave / invite / ban 等）。
  - `?not_membership=<value>` — 指定 membership を除外する。
  - `db::rooms::get_members_filtered()` 新設（動的 WHERE 句で両フィルタを組み合わせ可能）。

## v0.33.0 でやったこと

- **`GET /timestamp_to_event`** (`server/api/client/timestamp_to_event.rs`, `db/events.rs`):
  - `/_matrix/client/v1/rooms/{roomId}/timestamp_to_event?ts=<ms>&dir=f|b` — MSC3030 実装。
  - `dir=f` → ts 以降で最も古いイベント、`dir=b` → ts 以前で最も新しいイベントを返す。
  - `db::events::get_closest_event()` 新設。`UNIX_TIMESTAMP(created_at) * 1000` で ms 比較。

- **スレッド `latest_event` 拡充** (`server/api/client/threads.rs`):
  - `/threads` の集計クエリに `latest_ev.event_id` を追加（`MAX(stream_ordering)` と JOIN）。
  - `unsigned.m.relations.m.thread.latest_event` に完全なイベントオブジェクトを含めるよう変更。

- **`/aliases` に canonical_alias を含める** (`server/api/client/room_aliases.rs`):
  - `m.room.canonical_alias` 状態イベントの `alias` / `alt_aliases` を重複なしで `aliases` 配列に追加。

## v0.32.0 でやったこと

- **`GET /rooms/{roomId}/threads`** (`server/api/client/threads.rs`):
  - `/_matrix/client/v1/rooms/{roomId}/threads` — スレッドルート一覧を最新活動順で返す。
  - `event_relations` テーブルの `rel_type = 'm.thread'` を集計してスレッドルートを特定。
  - `?from=<stream_ordering>&limit=<n>` カーソルページネーション。
  - `?include=participated` フィルタ — 自分が投稿したスレッドのみ返す。
  - 各イベントの `unsigned.m.relations.m.thread` に `count` / `latest_event` / `current_user_participated` を付与。

- **`GET /rooms/{roomId}/aliases`** (`server/api/client/room_aliases.rs`):
  - `/_matrix/client/v3/rooms/{roomId}/aliases` — ルームに紐づくエイリアス一覧を返す。
  - `db::room_aliases::list_for_room()` 新設（ORDER BY alias 昇順）。

## v0.31.0 でやったこと

- **`unsigned.m.relations` 集計** (`db/relations.rs`, `db/events.rs`):
  - `relations::get_aggregations_batch(pool, event_ids)` を新設。複数 event_id を一括で集計。
  - **m.replace**: 最新の置換イベント（event_id / sender / origin_server_ts）を返す。
  - **m.reaction**: emoji key ごとの件数リスト `{"chunk": [{"type": "m.reaction", "key": "👍", "count": 3}]}` を返す。
  - `db::events::get_by_id()` — `unsigned.m.relations` を付与して返す。
  - `db::events::get_messages()` — 全イベントを一括集計して `unsigned.m.relations` を付与。
  - `db::events::get_context()` — 前後イベントに同様の集計を付与。

## v0.30.0 でやったこと

- **イベントリレーション記録** (`db/events.rs`, `schema.sql`):
  - `event_relations` テーブルを新設。`events.send()` で `m.relates_to` が含まれる場合に INSERT IGNORE。
  - rel_type / relates_to_event_id を保存し、`GET /relations` の応答源として使用。

- **`GET /relations`** (`server/api/client/relations.rs`, `db/relations.rs`):
  - `/_matrix/client/v1/rooms/{roomId}/relations/{eventId}[/{relType}[/{eventType}]]` の 3 バリアント。
  - stream_ordering ASC で並べ、`?from=<event_id>&limit=<n>` によるカーソルページネーション。
  - `chunk` / `next_batch` / `prev_batch` を返す。

- **`POST /read_markers`** (`server/api/client/read_markers.rs`):
  - `m.read` / `m.read.private` / `m.fully_read` を 1 リクエストで設定。
  - `m.read` 送信時は通知既読（`notifications.mark_read_up_to`）+ ハイライトクリア（`unread_highlights`）も同時実行。
  - `m.fully_read` は room account_data に保存。

## v0.29.0 でやったこと

- **`GET /notifications`** (`server/api/client/notifications.rs`, `db/notifications.rs`):
  - プッシュ通知履歴を返す新エンドポイント。
  - `notifications` テーブル新設（`schema.sql`）。`dispatch_push` が notify アクション発火時に INSERT。
  - `?from=<id>&limit=<n>` で ID ベースページネーション、`?only=highlight` でハイライトのみフィルタ。
  - `only=highlight` 時は `unread_highlights` テーブルと突き合わせて判定。

- **`/search` のページネーション** (`server/api/client/search.rs`, `db/events.rs`):
  - `?next_batch=<stream_ordering>` クエリパラメータを追加。
  - `search_room_events()` に `before_ordering: Option<i64>` を追加し、カーソル方式のページネーションを実装。

- **receipt POST 時の cleanup** (`server/api/client/receipts.rs`, `db/unread.rs`):
  - `m.read` / `m.read.private` 送信時に `notifications.mark_read_up_to()` で通知を既読にする。
  - 同時に `unread::delete_highlights_up_to()` でハイライトレコードを削除。

## 既知の課題・技術的負債

| 項目 | 詳細 |
|---|---|
| UIA ステージ m.login.password のみ | Matrix 仕様では他ステージ（m.login.sso 等）も定義されているが未対応 |
| TypingStore はサーバー再起動でリセット | インメモリのため永続化なし（Matrix 仕様上は許容範囲） |
| 非マクロ sqlx クエリ | 多数のモジュールが `sqlx::query()` 非マクロを使用（`.sqlx/` メタデータ未生成） |
| E2EE は鍵交換のみ | Olm セッション確立や Megolm グループセッション管理はクライアント側実装 |
| 状態解決アルゴリズム v2 未完全 | auth_events / prev_events は DB に保存されるが、グラフを使った完全な conflict resolution は未実装 |
| account_data since のクロックスキュー | `now_ms` はサーバー時刻のため、time-skew でごく稀に差分漏れの可能性 |
| depth の競合リスク | get_room_tip() と send() の間に他のイベントが挿入された場合、depth が重複する可能性 |
| /relations のページネーション | prev_batch は from トークンをそのまま返すのみ（後方ページングは未実装） |
| 3pid バリデーションなし | identity server 連携なし。登録は直接 INSERT のみ（メール確認なし） |
| login_tokens クリーンアップ | `purge_expired()` は実装済みだが定期実行はなし（起動時 or cron での呼び出しが必要） |
| lazy_load_members の初回 sync | 初回 sync 時に既訪問 member を追跡するクライアントキャッシュとの整合は未対応 |
| /hierarchy の cross-server 展開 | federation ルームの子は room_state に m.space.child がない場合スキップされる |
| admin API の認証強化 | 管理者トークン（Bearer admin-token 等）によるヘッダー認証は未対応。現状は `admin=1` フラグのみで判定 |
| device_lists.changed の粒度 | account_data_since_ms でフィルタしているため since トークン精度に依存する（ミリ秒→秒変換のため微小な漏れあり） |

## v0.48.0 候補

1. **状態解決アルゴリズム v2 完全実装** — auth_events + prev_events グラフを使った完全な conflict resolution
2. **複数 OIDC プロバイダー対応** — 複数の OIDC プロバイダーを同時設定（例: Google + Keycloak）
3. **メッセージ送信 power level 確認** — `m.room.power_levels` の `events_default` に基づいた送信権限チェック
4. **`/sync` の `rooms.leave` 拡充** — leave 時の timeline に `m.room.member` イベントを含め、クライアントが退室理由を表示できるようにする
5. **`/_matrix/client/v3/rooms/{roomId}/state` (room state snapshot) の最適化** — 現状は全イベントを返すが、最新 (type, state_key) ペアのみ返すよう変更

## 開発フロー（おさらい）

```sh
# 環境起動（DB）
docker compose up -d db
docker compose run --rm migrate

# ホストでサーバ起動
cargo run --bin server

# スキーマ変更時
#   1. schema/schema.sql を編集
#   2. dry-run で確認
./dev schema-dry-run
#   3. 適用
./dev schema-apply

# sqlx offline 用メタデータ再生成（クエリ変更時）
DATABASE_URL=... cargo sqlx prepare --workspace

# テスト
SQLX_OFFLINE=true cargo test

# フォーマット
cargo fmt
```

## 環境設定

- `.env.example` → `.env` にコピーしてパスワードを設定（`.env` は gitignore 済み）
- `DB_ROOT_PASS` が必須（MariaDB コンテナ起動時）
- `MEDIA_BACKEND=s3` + `S3_BUCKET` で S3 に切り替え可能（`--features server/s3` でビルド）
- `SQLX_OFFLINE=true` で DB なしビルド可能（`.sqlx/` がコミット済みのため CI でも動作）

## ブランチ戦略

- `main` — リリース済みタグのみマージ（マージ後 release.yml が自動でタグ・GitHub Release・次バージョンブランチを作成）
- `feature/v0.x.0` — バージョン単位の作業ブランチ
