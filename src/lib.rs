//! Typed Serde data access for Firebase Realtime Database.
//!
//! `rtdb-typed` is intentionally a thin layer over [`rtdb_rs`]. The underlying
//! crate owns Firebase REST transport, authentication, query construction, and
//! SSE behavior; this crate adds conversion between Firebase JSON and Rust
//! application models.
//!
//! Collection methods use [`FirebaseCollection`] for Firebase object maps. A
//! missing collection (`null`) becomes an empty collection through
//! [`TypedClient::get_collection`], while
//! [`TypedClient::get_optional_collection`] preserves the distinction as
//! `None`. Query methods provide the same choices through
//! [`TypedQuery::send_collection`] and [`TypedQuery::send_optional`].

mod client;
mod error;

pub use client::{FirebaseCollection, PushResult, TypedClient, TypedEvent, TypedQuery};
pub use error::TypedError;

pub use rtdb_rs;
