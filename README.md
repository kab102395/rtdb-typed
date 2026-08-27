# rtdb-typed

Typed Serde data access for Firebase Realtime Database, built on [`rtdb-rs`](https://github.com/kab102395/rtdb-rs).

`rtdb-typed` keeps Firebase transport in `rtdb-rs` while giving applications a strongly typed layer for models, collections, queries, partial patches, and realtime events. It is intentionally not an ORM and does not own authentication or synchronized application state.

## RTDB Rust ecosystem

```text
application
   |
   +-- rtdb-sync
   |     synchronized state, durability, offline queue/replay
   |
   +-- rtdb-typed
   |     Serde models, typed CRUD, collections, queries, realtime events
   |
   +-- rtdb-admin
   |     service-account auth and token lifecycle
   |
   `-- rtdb-rs
         Firebase REST + query + SSE transport
                  |
                  v
          Firebase Realtime Database
```

| Crate | Responsibility |
| --- | --- |
| [`rtdb-rs`](https://github.com/kab102395/rtdb-rs) | Raw Firebase REST/query/SSE transport |
| [`rtdb-typed`](https://github.com/kab102395/rtdb-typed) | Typed Serde CRUD, collections, queries, patches, and realtime events |
| [`rtdb-admin`](https://github.com/kab102395/rtdb-admin) | Service-account credentials and OAuth token lifecycle |
| [`rtdb-sync`](https://github.com/kab102395/rtdb-sync) | Synchronized application state, local writes, durability, offline replay, reconciliation |

`rtdb-typed` can be used directly for applications that want typed Firebase access without maintained local synchronization state. `rtdb-sync` builds on this layer for long-lived synchronized state.

## Current release line

The current development package is `0.3.0`.

Its realtime model deliberately distinguishes complete Firebase values from partial updates:

- `TypedEvent::Put` carries `Option<T>`; `None` represents Firebase `null`/deletion.
- `TypedEvent::Patch` carries `TypedPatch`, preserving only fields actually changed by Firebase.
- `KeepAlive` and `Cancel` remain visible to callers.

This replaces the older provisional shape that treated a partial Firebase patch as if it were a complete `T`.

## Core API

The typed client supports:

- `TypedClient::get<T>()`
- `TypedClient::put<T>()`
- `TypedClient::patch<T>()`
- `TypedClient::post<T>()`
- `TypedClient::delete()`
- `TypedClient::get_collection<T>()`
- `TypedClient::get_optional_collection<T>()`
- `TypedClient::query<T>().send()`
- `TypedClient::query<T>().send_optional()`
- `TypedClient::query<T>().send_collection()`
- `TypedClient::stream()`
- `TypedClient::query(...).stream()`

The layer delegates HTTP, URL construction, Firebase query semantics, namespaces, authentication parameters, and SSE transport to `rtdb-rs`.

## Typed models

Any compatible Serde model can be used directly.

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct User {
    name: String,
    score: u64,
}
```

The typed layer serializes writes through `serde::Serialize` and deserializes reads through `serde::Deserialize`.

## Collections

Firebase object maps are represented as `FirebaseCollection<T>`.

Collection helpers include typed:

- `len`
- `is_empty`
- `get`
- `contains_key`
- `keys`
- `values`
- `iter`
- `into_inner`

`get_collection` and query `send_collection` convert Firebase `null` into an empty collection. Optional collection variants preserve `null` as `None`.

Push writes return a typed `PushResult { key, path }`.

## Typed queries

Typed query builders preserve the underlying Firebase query rules from `rtdb-rs` while deserializing the response into application types.

This allows applications to use Firebase ordering/filtering without manually decoding `serde_json::Value` at every call site.

## Typed realtime events

Direct and filtered SSE streams are converted from `rtdb-rs::RtdbEvent` into typed events.

`Put` is a complete replacement at the relevant Firebase path:

```text
TypedEvent::Put { data: Some(model), ... }
```

Deletion/null is represented explicitly:

```text
TypedEvent::Put { data: None, ... }
```

A Firebase patch is not assumed to contain a complete model:

```text
TypedEvent::Patch { data: TypedPatch, ... }
```

`TypedPatch` can deserialize individual changed fields or apply supported shallow changes to an existing typed model. This distinction is important for synchronization layers because partial Firebase updates must not silently erase fields that were not present in the patch.

## Relationship to rtdb-sync

[`rtdb-sync`](https://github.com/kab102395/rtdb-sync) uses typed model/event conversion while owning the higher-level state-management concerns:

- hydration
- current local snapshot
- watch/subscriber notifications
- reconnect policy
- local PUT/PATCH writes
- acknowledgement and echo handling
- conflict policy
- durable snapshots
- offline mutation journals
- process restart recovery
- replay/reconciliation after reconnect

Those behaviors intentionally remain outside `rtdb-typed`.

## Relationship to rtdb-admin

[`rtdb-admin`](https://github.com/kab102395/rtdb-admin) owns service-account loading, JWT/OAuth exchange, token expiry, refresh, and authenticated `rtdb-rs` client lifecycle. `rtdb-typed` consumes the resulting transport client rather than storing service-account credentials itself.

## Testing

The project uses several validation layers:

1. serialization/deserialization and conversion unit tests with no network
2. localhost mock HTTP/SSE tests for deterministic transport-boundary behavior
3. optional official Firebase Realtime Database Emulator tests for real local Firebase behavior
4. ecosystem integration through `rtdb-sync`, where typed operations run concurrently with raw transport, admin-authenticated clients, synchronized local writes, subscribers, offline replay, and long-duration stress profiles

The emulator coverage includes typed CRUD, queries, realtime SSE behavior, filtered-child behavior, and fan-out scenarios. No production Firebase account is required for the normal local suite.

## Development status

The major typed layers are implemented:

- typed CRUD
- collections
- optional/null collection semantics
- typed queries
- typed direct and filtered realtime streams
- explicit deletion handling
- `TypedPatch` partial-update semantics
- local mock validation
- official emulator validation

The package remains pre-1.0, so public API changes are still possible.

Before coordinated publication, dependency metadata is being aligned with the matching published `rtdb-rs` release and local workspace patches are removed for the final crates.io graph.

## Scope

`rtdb-typed` is a typed data layer, not an ORM, authentication manager, synchronization engine, or offline database. Those responsibilities are intentionally separated into the other ecosystem crates.

## License

MIT
