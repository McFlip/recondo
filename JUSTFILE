check:
  cargo check
  cargo clippy
  cargo test
  pnpm exec playwright test

run: check
  rm -f ui/web/db/graph.redb
  rm -f assets/*
  cargo run
