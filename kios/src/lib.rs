//! KIOS library surface.
//!
//! Layers:
//! - [`domain`] — pure types and rules (no I/O)
//! - [`application`] — use-cases / orchestration
//! - [`infrastructure`] — adapters (I/O, external systems)
//! - [`presentation`] — CLI / UI entry points

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
