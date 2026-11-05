# AGENTS.md

## Commands
- Full verification: `just check` runs `cargo check`, `cargo clippy`, `cargo test`, then `pnpm exec playwright test`.
- Rust only: `cargo check`, `cargo clippy`, `cargo test`.
- Single crate: `cargo test -p recondo-domain`, `-p recondo-dal`, or `-p recondo-web`.
- Single Rust test: `cargo test -p recondo-domain <test_name_substring>`.
- Single Playwright test: `pnpm exec playwright test tests/explore.spec.ts` or `pnpm exec playwright test -g "pattern"`.
- Run app: `just run` (runs full check, then deletes `ui/web/db/graph.redb` and `assets/*`, then `cargo run`). Or `cargo run -p recondo-web` to keep data.
- E2E suite: `pnpm exec playwright test` (or `pnpm test`) from repo root; Playwright builds/runs the Rust server itself in a temp dir.

## Layout
- `domain/` — pure domain model, traits (`MediaRepository`, `TagRepository`), commands, and queries.
- `dal/` — redb persistence implementing both traits.
- `ui/web/` — axum web app (maud + datastar); pages in `src/pages/`, static assets in `static/`.
- `tests/` — Playwright E2E tests.

## Runtime / environment
- `RECONDO_DB_PATH` (default `ui/web/db/graph.redb`); parent dirs created on demand.
- `RECONDO_ADDR` (default `127.0.0.1:3000`).
- `RECONDO_MAX_UPLOAD_MB` — unset means unlimited; invalid value logs a warning and stays unlimited.
- `RECONDO_E2E_PORT` (default `3199`) — Playwright server port.
- Paths are relative to the process working directory; run from repo root. `assets/` and `ui/web/db/` are gitignored.
- `just run` wipes the local DB and `assets/` before starting.

## Data model quirks
- Media ID is the BLAKE3 hex digest of file content; identical uploads are detected as duplicates.
- Every upload gets a default tag pair: relationship `to do`, tag `needs tags`.
- Tag and relationship names are normalized (trim, lowercase, NFC); tag IDs are BLAKE3 of the normalized name.
- On duplicate upload with selected tags, those tags are added; the default pair is removed only if it was the media's sole pair.
- Media details page prevents removing the last remaining tag.

## Testing notes
- Rust tests need no external services; DAL tests use temp dirs.
- Playwright starts its own server in a temp dir, so DB/assets stay out of the repo; do not start a separate server for E2E.
- Static files under `ui/web/static/` and demo images under `ui/web/tests/fixtures/` are embedded at compile time (`include_str!`/`include_bytes!`); edits require a rebuild.

## Conventions
- redb is synchronous, so DB work in handlers is dispatched with `tokio::task::spawn_blocking`.
- Web state holds `Arc<RedbMediaRepository>` and an `assets_dir` (`PathBuf::from("assets")`).
- Asset serving validates that the resolved path stays under `assets/` and that the file stem equals the media ID.
- Prefer Datastar reactivity (signals, `data-on:*`, SSE patch endpoints) over full page reloads when updating UI state.