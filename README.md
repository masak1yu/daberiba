# daberiba — Matrix Platform

A [Matrix](https://matrix.org/) protocol-compliant platform — homeserver backend (and planned frontend client).

**Status:** v0.9.0 — Client-Server API Phase 9 (functional, not production-ready)

[![CI](https://github.com/masak1yu/daberiba/actions/workflows/ci.yml/badge.svg)](https://github.com/masak1yu/daberiba/actions/workflows/ci.yml)

## Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust |
| Web framework | Axum 0.7 |
| Database | MariaDB 11 |
| Schema management | [sqldef (mysqldef)](https://github.com/sqldef/sqldef) |
| Container runtime | Podman |
| Task runner | [just](https://github.com/casey/just) (or `./dev` — no install required) |

## Implemented Endpoints

### Public
| Method | Path | Description |
|---|---|---|
| GET | `/_matrix/client/versions` | Supported spec versions |
| GET | `/_matrix/client/v3/login` | Login flows |
| POST | `/_matrix/client/v3/login` | Login (m.login.password) |
| POST | `/_matrix/client/v3/register` | Register |
| GET | `/_matrix/client/v3/capabilities` | Server capabilities |
| GET | `/.well-known/matrix/client` | Client discovery |
| GET | `/.well-known/matrix/server` | Server discovery |

### Authenticated
| Method | Path | Description |
|---|---|---|
| GET | `/_matrix/client/v3/account/whoami` | Current user |
| POST | `/_matrix/client/v3/account/password` | Change password (UIA) |
| POST | `/_matrix/client/v3/logout` | Logout |
| POST | `/_matrix/client/v3/logout/all` | Logout all devices |
| GET | `/_matrix/client/v3/sync` | Sync (stream_ordering cursor, ephemeral events) |
| GET | `/_matrix/client/v3/devices` | List devices |
| GET | `/_matrix/client/v3/devices/{deviceId}` | Get device |
| PUT | `/_matrix/client/v3/devices/{deviceId}` | Update device display name |
| DELETE | `/_matrix/client/v3/devices/{deviceId}` | Delete device |
| POST | `/_matrix/client/v3/delete_devices` | Bulk delete devices (UIA) |
| POST | `/_matrix/client/v3/createRoom` | Create room |
| POST | `/_matrix/client/v3/join/{roomId}` | Join room |
| POST | `/_matrix/client/v3/rooms/{roomId}/leave` | Leave room |
| GET | `/_matrix/client/v3/joined_rooms` | List joined rooms |
| PUT | `/_matrix/client/v3/rooms/{roomId}/send/{type}/{txnId}` | Send event |
| GET | `/_matrix/client/v3/rooms/{roomId}/messages` | Message history (paginated) |
| PUT | `/_matrix/client/v3/rooms/{roomId}/state/{type}` | Send state event |
| PUT | `/_matrix/client/v3/rooms/{roomId}/state/{type}/{key}` | Send state event (with key) |
| GET | `/_matrix/client/v3/rooms/{roomId}/state` | Get room state |
| GET | `/_matrix/client/v3/rooms/{roomId}/state/{type}/{key}` | Get state event |
| GET | `/_matrix/client/v3/rooms/{roomId}/members` | Room members |
| GET | `/_matrix/client/v3/rooms/{roomId}/joined_members` | Joined members |
| POST | `/_matrix/client/v3/rooms/{roomId}/invite` | Invite user |
| POST | `/_matrix/client/v3/rooms/{roomId}/receipt/{type}/{eventId}` | Send read receipt |
| PUT | `/_matrix/client/v3/rooms/{roomId}/typing/{userId}` | Set typing indicator |
| GET/PUT | `/_matrix/client/v3/profile/{userId}` | User profile |
| GET/PUT | `/_matrix/client/v3/profile/{userId}/displayname` | Display name |
| GET/PUT | `/_matrix/client/v3/profile/{userId}/avatar_url` | Avatar URL |
| GET | `/_matrix/client/v3/pushers` | List pushers |
| POST | `/_matrix/client/v3/pushers/set` | Register / delete pusher |
| GET | `/_matrix/client/v3/publicRooms` | Public room directory |
| PUT | `/_matrix/client/v3/directory/room/{roomAlias}` | Create room alias |
| GET | `/_matrix/client/v3/directory/room/{roomAlias}` | Resolve room alias |
| DELETE | `/_matrix/client/v3/directory/room/{roomAlias}` | Delete room alias |
| PUT | `/_matrix/client/v3/presence/{userId}/status` | Set presence status |
| GET | `/_matrix/client/v3/presence/{userId}/status` | Get presence status |
| GET | `/_matrix/client/v3/user/{userId}/rooms/{roomId}/tags` | Get room tags |
| PUT | `/_matrix/client/v3/user/{userId}/rooms/{roomId}/tags/{tag}` | Set room tag |
| DELETE | `/_matrix/client/v3/user/{userId}/rooms/{roomId}/tags/{tag}` | Delete room tag |
| POST | `/_matrix/client/v3/user/{userId}/filter` | Create filter |
| GET | `/_matrix/client/v3/user/{userId}/filter/{filterId}` | Get filter |
| PUT | `/_matrix/client/v3/sendToDevice/{eventType}/{txnId}` | Send to-device message |
| POST | `/_matrix/client/v3/keys/upload` | Upload device keys / one-time keys |
| POST | `/_matrix/client/v3/keys/query` | Query device keys for users |
| POST | `/_matrix/client/v3/keys/claim` | Claim one-time keys |
| POST | `/_matrix/media/v3/upload` | Upload media |
| GET | `/_matrix/media/v3/download/{serverName}/{mediaId}` | Download media |

## Getting Started

### GitHub Codespaces (iPad Pro など)

[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/masak1yu/daberiba)

Codespace を開くだけで以下が自動セットアップされます。

- Rust toolchain、`just`、`mysqldef` インストール済み
- MariaDB 起動済み
- `claude` コマンド (`@anthropic-ai/claude-code`) インストール済み
- ポート `8448` を自動フォワード
- zsh プロンプトにブランチ名・変更状態を表示 (Oh My Zsh `robbyrussell`)

```sh
# Codespace ターミナルで
just up          # DBマイグレーション
just dev         # サーバー起動 → ポート8448がブラウザから開く
claude           # Claude Code 起動
```

> **Note:** `.env` は開発用デフォルトパスワードで自動生成されます。

---

### ローカル開発

#### Requirements

- [Podman](https://podman.io/) + podman-compose

That's it. `just` and `mysqldef` are bundled in the tools container.

### Setup

```sh
# Copy environment config and fill in passwords
cp .env.example .env

# Start DB and apply schema automatically
podman compose up -d db migrate

# Run the server (on host)
cargo run --bin server
```

The server listens on `0.0.0.0:8448` by default.

### Running commands without just installed

```sh
./dev schema-apply     # Apply schema changes
./dev schema-dry-run   # Preview schema changes
./dev --list           # Show all available recipes
```

### With just installed

```sh
just up            # Start DB + auto schema apply
just dev           # Run server on host
just test          # Run tests
just schema-apply  # Apply schema via tools container
just shell         # Open shell in tools container
```

## Configuration

Copy `.env.example` to `.env` and adjust as needed.

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | — | MariaDB connection string |
| `BIND_ADDR` | `0.0.0.0:8448` | Server listen address |
| `SERVER_NAME` | `localhost` | Matrix server name (used in user/room IDs) |
| `CORS_ORIGINS` | `*` | Allowed CORS origins (`*` or comma-separated URLs) |
| `MEDIA_BACKEND` | `local` | Media storage backend: `local` or `s3` (requires `--features server/s3`) |
| `MEDIA_PATH` | `./media` | Local media file storage directory |
| `S3_BUCKET` | — | S3 bucket name (required when `MEDIA_BACKEND=s3`) |
| `AWS_REGION` | — | AWS region (S3) |
| `AWS_ACCESS_KEY_ID` | — | AWS access key (S3) |
| `AWS_SECRET_ACCESS_KEY` | — | AWS secret key (S3) |
| `AWS_ENDPOINT_URL` | — | Custom endpoint for S3-compatible storage (e.g. MinIO) |
| `RUST_LOG` | `server=debug,tower_http=debug` | Log level |

> **Note:** The local DB is mapped to port `13306` to avoid conflicts with any locally running MySQL on `3306`.

## Project Structure

```
daberiba/
├── crates/
│   ├── server/          # Axum HTTP server
│   │   └── src/
│   │       ├── api/
│   │       │   ├── client/   # Matrix Client-Server API handlers
│   │       │   └── media.rs  # Matrix Media API handlers
│   │       ├── media_store.rs  # MediaStore trait + LocalStore + S3Store
│   │       ├── middleware/   # Auth (Bearer token) + last_seen update
│   │       ├── typing_store.rs # TypingStore (in-memory, DashMap + TTL)
│   │       ├── uia.rs        # User Interactive Authentication (UiaStore)
│   │       ├── router.rs
│   │       ├── state.rs
│   │       └── error.rs      # Matrix-compliant error responses
│   └── db/              # sqlx database layer
│       └── src/         # users, rooms, events, sync, profile, devices, media, pushers, receipts
├── frontend/            # (planned) Matrix frontend client
├── schema/
│   └── schema.sql        # Managed by sqldef (mysqldef)
├── .sqlx/                # sqlx offline query cache (committed)
├── .devcontainer/        # GitHub Codespaces 設定
├── Dockerfile            # Server image
├── Dockerfile.tools      # just + mysqldef tools image (arch auto-detect)
├── compose.yml           # podman compose (db, migrate, tools, server)
├── justfile
└── dev                   # ./dev <recipe> — runs just via tools container
```

## UIA (User Interactive Authentication)

`POST /account/password` and `POST /delete_devices` require UIA with `m.login.password`.

**Flow:**
1. Send request without `auth` → server returns `401` with `flows` and `session` (5-minute TTL)
2. Re-send with `auth.type = "m.login.password"`, `auth.password`, and `auth.session`

## Pagination (`/messages`)

```
GET /rooms/{roomId}/messages?dir=b&limit=20
→ { "chunk": [...], "start": "s100", "end": "s81" }

GET /rooms/{roomId}/messages?from=s81&dir=b&limit=20
→ next page (older events)
```

Token format: `s{stream_ordering}` (same as `/sync` cursor). `end` is absent when no more events exist.

## Push Notifications

Register an HTTP pusher via `POST /pushers/set`:

```json
{
  "app_id": "com.example.app",
  "pushkey": "<device-token>",
  "kind": "http",
  "app_display_name": "My App",
  "device_display_name": "My Phone",
  "lang": "en",
  "data": { "url": "https://push.example.com/_matrix/push/v1/notify" }
}
```

When an event is sent to a room, the server dispatches HTTP push notifications to all room members' registered pushers (best-effort, non-blocking). Use `kind: null` to delete a pusher.

## Read Receipts

Send a read receipt via `POST /rooms/{roomId}/receipt/m.read/{eventId}`.

Receipts are returned in `/sync` responses as `m.receipt` ephemeral events:

```json
{
  "type": "m.receipt",
  "content": {
    "$event_id": {
      "m.read": { "@user:server": { "ts": 1234567890 } }
    }
  }
}
```

## Typing Indicators

```json
PUT /rooms/{roomId}/typing/{userId}
{ "typing": true, "timeout": 30000 }
```

Active typing users are returned in `/sync` as `m.typing` ephemeral events. State is in-memory and resets on server restart.

## Public Rooms

`GET /publicRooms` returns rooms where `m.room.join_rules` state is set to `"public"`.

## Unread Notification Counts

`/sync` responses include `unread_notifications` per joined room:

```json
"unread_notifications": {
  "notification_count": 5,
  "highlight_count": 1
}
```

`notification_count` is events after the user's last `m.read` receipt. `highlight_count` is the subset that mention the user.

## Room Aliases

```sh
# Create
PUT /_matrix/client/v3/directory/room/%23alias%3Aserver  {"room_id": "!abc:server"}

# Resolve
GET /_matrix/client/v3/directory/room/%23alias%3Aserver
→ {"room_id": "!abc:server", "servers": ["server"]}

# Join by alias
POST /_matrix/client/v3/join/%23alias%3Aserver
```

## Presence

```sh
# Set
PUT /_matrix/client/v3/presence/@user:server/status
{"presence": "online", "status_msg": "In a meeting"}

# Get
GET /_matrix/client/v3/presence/@user:server/status
→ {"presence": "online", "last_active_ago": 1234, "currently_active": true}
```

Presence events for joined room members are included in `/sync` as `m.presence` events in the top-level `presence.events` array.

## Room Tags

```sh
# Set a tag
PUT /_matrix/client/v3/user/@user:server/rooms/!room:server/tags/m.favourite
{"order": 0.5}

# Get tags
GET /_matrix/client/v3/user/@user:server/rooms/!room:server/tags
→ {"tags": {"m.favourite": {"order": 0.5}}}

# Delete a tag
DELETE /_matrix/client/v3/user/@user:server/rooms/!room:server/tags/m.favourite
```

Tags are returned in `/sync` per joined room as `account_data` events with type `m.tag`.

## Filters

```sh
# Create a filter
POST /_matrix/client/v3/user/@user:server/filter
{"room": {"timeline": {"types": ["m.room.message"]}}}
→ {"filter_id": "1"}

# Retrieve a filter
GET /_matrix/client/v3/user/@user:server/filter/1

# Use a filter in /sync
GET /_matrix/client/v3/sync?filter=1
```

The `filter` query parameter accepts a filter ID or an inline JSON filter. The following filter fields are supported:

| Field | Effect |
|---|---|
| `room.rooms` / `room.not_rooms` | Include / exclude specific rooms |
| `room.timeline.types` / `not_types` | Filter timeline events by type |
| `room.state.types` / `not_types` | Filter state events by type |
| `room.ephemeral.types` / `not_types` | Filter ephemeral events by type |
| `room.account_data.types` / `not_types` | Filter per-room account_data events |
| `presence.types` / `not_types` | Filter presence events |

## To-Device Messages

```sh
PUT /_matrix/client/v3/sendToDevice/m.room.key/txn1
{
  "messages": {
    "@bob:server": {
      "*": { "algorithm": "m.megolm.v1.aes-sha2", ... }
    }
  }
}
```

Pending to-device events are returned in `/sync` under `to_device.events` and deleted after delivery.

## Invite Flow

When a user is invited to a room, the server:

1. Records the `invited_by` (inviter user ID) in `room_memberships`
2. Dispatches an HTTP push notification to the invitee's registered pushers (best-effort, non-blocking)
3. Returns the invite in `/sync` as `rooms.invite` with stripped state events:
   - `m.room.name` (if set)
   - `m.room.member` for the inviter (membership: join)
   - `m.room.member` for the invitee (membership: invite)

## E2EE Key Management

Supports Olm/Megolm key exchange endpoints:

```sh
# Upload device keys and one-time keys
POST /_matrix/client/v3/keys/upload
{
  "device_keys": { "user_id": "...", "device_id": "...", "algorithms": [...], "keys": {...}, "signatures": {...} },
  "one_time_keys": { "curve25519:AAAAAA": "..." }
}
→ {"one_time_key_counts": {"curve25519": 5}}

# Query device keys for users
POST /_matrix/client/v3/keys/query
{"device_keys": {"@alice:server": []}}
→ {"device_keys": {"@alice:server": {"DEVICE_ID": {...}}}}

# Claim one-time keys
POST /_matrix/client/v3/keys/claim
{"one_time_keys": {"@alice:server": {"DEVICE_ID": "curve25519"}}}
→ {"one_time_keys": {"@alice:server": {"curve25519:AAAAAA": "..."}}}
```

## Not Yet Implemented

- Federation (`/_matrix/federation`) — out of scope for now

## License

TBD
