#![deny(warnings)]

// Minimal library target that exposes the JS runtime so that external crates
// in this workspace (currently `benches/js-runtime-perf`) can link against the
// real implementation. The main binary at `src/main.rs` still owns the full
// module tree for the server and is unaffected by this target.
pub mod js_runtime;
