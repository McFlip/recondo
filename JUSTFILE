check:
  cargo check
  cargo clippy
  cargo test

run: check
  rm -f ui/web/db/graph.redb
  rm -f assets/*
  cargo run
