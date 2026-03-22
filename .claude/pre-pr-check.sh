#!/usr/bin/env bash
# Pre-PR hook: runs automatically before `gh pr create`
# Checks: cargo fmt, cargo test
# Reminds: Cargo.toml version bump, README, handover.md

cmd=$(jq -r '.tool_input.command // ""')

if ! echo "$cmd" | grep -qE "gh pr create"; then
  exit 0
fi

cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)" || exit 0

# 1. fmt check (stdout/stderr → /dev/null; only JSON goes to stdout)
if ! SQLX_OFFLINE=true cargo fmt --check >/dev/null 2>&1; then
  printf '{"continue":false,"stopReason":"Blocked: cargo fmt check failed — run `cargo fmt` first"}'
  exit 0
fi

# 2. tests
if ! SQLX_OFFLINE=true cargo test >/dev/null 2>&1; then
  printf '{"continue":false,"stopReason":"Blocked: cargo test failed — fix tests before creating PR"}'
  exit 0
fi

# 3. all passed — remind about manual steps
printf '{"systemMessage":"Pre-PR checks passed (fmt \u2713, tests \u2713).\nReminder: Cargo.toml version bumped? README updated? handover.md updated?"}'
