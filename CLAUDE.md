# daberiba — Development Rules for Claude

## Language Rules

| Artifact | Language |
|---|---|
| `README.md` | **English** |
| `handover.md` | **Japanese** |
| Git commit messages | **English** |
| PR title & body (`gh pr create`) | **English** |
| Code comments | Japanese (inline) or English — be consistent within a file |

## Before Creating a PR

The following must be completed before running `gh pr create`:

1. **`cargo fmt`** — format all code
2. **`cargo clippy --all-targets -- -D warnings`** — no warnings, no errors
3. **`cargo test`** — all tests pass
4. **Cargo.toml version bump** — bump `version` in both `crates/db/Cargo.toml` and `crates/server/Cargo.toml` to the next minor version (e.g. `0.6.0` → `0.7.0`)
5. **README.md** — update the status line (`**Status:** vX.Y.Z`) and the implemented endpoints table to reflect new endpoints
6. **handover.md** — rewrite for the next version: summarize what was done, update known issues, and list the next version's candidates

## Branch Strategy

- `main` — merged only when a version is released and tagged
- `feature/v0.X.0` — one branch per version; work happens here

## Schema Changes

When adding or modifying tables in `schema/schema.sql`:

1. Edit `schema/schema.sql`
2. Run `./dev schema-dry-run` to preview
3. Run `./dev schema-apply` to apply
4. Use non-macro `sqlx::query()` for new tables (no `.sqlx/` metadata yet)
5. Run `DATABASE_URL=... cargo sqlx prepare --workspace` after DB is live to regenerate `.sqlx/`

## Adding a New Endpoint

1. Add DB layer in `crates/db/src/<module>.rs`
2. Register the module in `crates/db/src/lib.rs`
3. Add API handler in `crates/server/src/api/client/<module>.rs`
4. Register the module in `crates/server/src/api/client/mod.rs`
5. Wire routes in `crates/server/src/router.rs`
6. Add the endpoint to the README table
