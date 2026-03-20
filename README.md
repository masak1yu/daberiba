# ba — Matrix Homeserver

A [Matrix](https://matrix.org/) protocol-compliant homeserver implementation.

**Status:** v0.1.0 — Client-Server API Phase 1 (functional, not production-ready)

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
| POST | `/_matrix/client/v3/logout` | Logout |
| POST | `/_matrix/client/v3/logout/all` | Logout all devices |
| GET | `/_matrix/client/v3/sync` | Sync (long-poll) |
| POST | `/_matrix/client/v3/createRoom` | Create room |
| POST | `/_matrix/client/v3/join/{roomId}` | Join room |
| POST | `/_matrix/client/v3/rooms/{roomId}/leave` | Leave room |
| GET | `/_matrix/client/v3/joined_rooms` | List joined rooms |
| PUT | `/_matrix/client/v3/rooms/{roomId}/send/{type}/{txnId}` | Send event |
| GET | `/_matrix/client/v3/rooms/{roomId}/messages` | Message history |
| PUT | `/_matrix/client/v3/rooms/{roomId}/state/{type}` | Send state event |
| PUT | `/_matrix/client/v3/rooms/{roomId}/state/{type}/{key}` | Send state event (with key) |
| GET | `/_matrix/client/v3/rooms/{roomId}/state` | Get room state |
| GET | `/_matrix/client/v3/rooms/{roomId}/state/{type}/{key}` | Get state event |
| GET | `/_matrix/client/v3/rooms/{roomId}/members` | Room members |
| GET | `/_matrix/client/v3/rooms/{roomId}/joined_members` | Joined members |
| POST | `/_matrix/client/v3/rooms/{roomId}/invite` | Invite user |
| GET/PUT | `/_matrix/client/v3/profile/{userId}` | User profile |
| GET/PUT | `/_matrix/client/v3/profile/{userId}/displayname` | Display name |
| GET/PUT | `/_matrix/client/v3/profile/{userId}/avatar_url` | Avatar URL |

## Getting Started

### Requirements

- [Podman](https://podman.io/) 5.x

That's it. `just` and `mysqldef` are bundled in the tools container.

### Setup

```sh
# Copy environment config
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
| `DATABASE_URL` | `mysql://matrix:matrix@127.0.0.1:13306/matrix` | MariaDB connection string |
| `BIND_ADDR` | `0.0.0.0:8448` | Server listen address |
| `SERVER_NAME` | `localhost` | Matrix server name (used in user/room IDs) |
| `CORS_ORIGINS` | `*` | Allowed CORS origins (`*` or comma-separated URLs) |
| `RUST_LOG` | `server=debug,tower_http=debug` | Log level |

> **Note:** The local DB is mapped to port `13306` to avoid conflicts with any locally running MySQL on `3306`.

## Project Structure

```
ba/
├── crates/
│   ├── server/          # Axum HTTP server
│   │   └── src/
│   │       ├── api/client/   # Matrix Client-Server API handlers
│   │       ├── middleware/   # Auth (Bearer token)
│   │       ├── router.rs
│   │       ├── state.rs
│   │       └── error.rs      # Matrix-compliant error responses
│   └── db/              # sqlx database layer
│       └── src/         # users, rooms, events, sync, profile, ...
├── schema/
│   ├── schema.sql        # Managed by sqldef (mysqldef)
│   └── justfile
├── Dockerfile            # Server image
├── Dockerfile.tools      # just + mysqldef tools image
├── compose.yml           # podman compose (db, migrate, tools, server)
├── justfile
└── dev                   # ./dev <recipe> — runs just via tools container
```

## Not Yet Implemented

- Device management (`/devices`)
- Media upload/download (`/_matrix/media`)
- Password change
- Push notifications
- Federation (`/_matrix/federation`) — out of scope for now

## License

TBD
