check:
  cargo check
  cargo clippy
  cargo test

run: check
  rm ui/web/db/graph.redb
  cargo run
