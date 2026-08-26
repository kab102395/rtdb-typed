//! Typed Serde data access for Firebase Realtime Database.
//!
//! `rtdb-typed` is intentionally a thin layer over [`rtdb_rs`]. The underlying
//! crate owns Firebase REST transport, authentication, query construction, and
//! SSE behavior; this crate adds conversion between Firebase JSON and Rust
//! application models.

mod client;
mod error;

pub use client::TypedClient;
pub use error::TypedError;

pub use rtdb_rs;
