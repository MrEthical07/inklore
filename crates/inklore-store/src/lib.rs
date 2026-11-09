//! Embedded storage engine: pager, copy-on-write B+tree, task event log, and the `Store` trait.
//! Knows nothing about code or knowledge: it stores bytes under keys.
//!
//! See `docs/05-storage.md` for the design.
