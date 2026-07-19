#![deny(warnings)]

// Minimal library target that exposes the JS runtime so that external crates
// in this workspace (currently `benches/js-runtime-perf`) can link against the
// real implementation. The main binary at `src/main.rs` still owns the full
// module tree for the server and is unaffected by this target.
pub mod js_runtime;

// The `js_runtime` module is compiled under both this lib target and the binary
// target. Its `#[sqlx::test(migrator = "crate::MIGRATOR")]` tests therefore need
// `crate::MIGRATOR` to resolve here too (the binary defines its own in `main.rs`).
#[cfg(test)]
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
