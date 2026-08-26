# rtdb-typed

Typed Serde data layer for Firebase Realtime Database, built on [`rtdb-rs`](https://crates.io/crates/rtdb-rs).

`rtdb-typed` is intended to keep Firebase transport concerns in `rtdb-rs` while adding a small, strongly typed API for application models.

## Goals

- Deserialize Firebase RTDB JSON directly into `serde::Deserialize` types.
- Serialize Rust values through `serde::Serialize` for writes.
- Preserve the path/query semantics of `rtdb-rs` instead of inventing an ORM.
- Add typed query and streaming adapters without duplicating the underlying HTTP/SSE implementation.
- Keep tests local and deterministic by default; no production Firebase database is required.

## API

The initial `0.1.x` line will focus on a narrow typed wrapper:

- `TypedClient::get<T>()`
- `TypedClient::put<T>()`
- `TypedClient::patch<T>()`
- `TypedClient::post<T>()`
- `TypedClient::delete()`
- `TypedClient::get_collection<T>()`
- `TypedClient::get_optional_collection<T>()`
- `TypedClient::query<T>().send()` for typed query results
- `TypedClient::query<T>().send_optional()` for nullable query results
- `TypedClient::query<T>().send_collection()` for typed Firebase object maps
- `TypedClient::query<T>().stream()` for typed SSE events

Collection methods return `FirebaseCollection<T>`, which provides typed
`len`, `is_empty`, `get`, `contains_key`, `keys`, `values`, `iter`, and
`into_inner` operations. `get_collection` and `send_collection` convert
Firebase `null` to an empty collection; optional variants return `None` for
`null`. Push writes return `PushResult { key, path }`.

`TypedEvent::Put` contains `Some(T)` for a complete value and `None` for a
Firebase deletion/null. `TypedEvent::Patch` contains `TypedPatch`, preserving
only the changed fields; it is never deserialized as a complete model. A patch
can inspect a field with `deserialize_field` or apply shallow changes to an
existing model with `apply_to`. Use `TypedClient::stream` for a direct stream,
or `TypedClient::query(...).stream()` for filtered streams; `Cancel` is
preserved and terminates the typed stream after delivery.

### 0.3 migration note

The provisional 0.1/0.2 realtime shape is replaced in 0.3: `Put` now carries
`Option<T>` (`None` means Firebase `null`), and `Patch` now carries
`TypedPatch` rather than pretending changed fields form a complete `T`.
Consumers should explicitly apply patches to their current model and handle
deletion before continuing synchronization.

## Testing strategy

### 1. Unit tests — no network

Pure serialization/deserialization and error-conversion tests run with `cargo test` and do not start any server.

### 2. Local mock HTTP tests — no Firebase account

Integration tests will use a local mock HTTP server to emulate Firebase REST responses. This validates the `rtdb-typed -> rtdb-rs -> HTTP` boundary without contacting Firebase or consuming billable database operations.

### 3. Firebase Realtime Database Emulator — optional end-to-end tests

Firebase's Local Emulator Suite includes a Realtime Database emulator. It can run entirely on localhost and is appropriate for integration/CI testing without touching production data or incurring production database usage. The preferred test project will use a `demo-` project ID so accidental fall-through to live Firebase resources is impossible.

The emulator remains an optional test layer; normal development should not require it.

## Development roadmap

### Phase 1 — foundation (implemented)

- crate metadata and dependency on `rtdb-rs`
- `TypedClient`
- typed error model
- typed CRUD methods
- serialization unit tests
- local HTTP mock integration tests

The local emulator smoke path is also implemented in `tests/emulator.rs` and
is run by `scripts/test-emulator.sh`. It verifies the emulator plus typed CRUD
without any Firebase account or production resource.

### Phase 2 — typed queries and collections (implemented)

- typed wrapper around `rtdb_rs::GetBuilder`
- `send<T>()`
- `FirebaseCollection<T>` helpers for Firebase key/value maps
- typed optional and collection query results
- query error tests

### Phase 3 — first-class typed realtime streams (implemented)

- map `RtdbEvent::Put` and `RtdbEvent::Patch` JSON payloads into typed events
- represent deletion as `TypedEvent::Put { data: None }`
- inspect/apply partial updates with `TypedPatch`
- preserve `KeepAlive` and `Cancel`
- test SSE parsing through a local mock server

### Phase 4 — emulator verification and CI

- Firebase Emulator configuration
- end-to-end CRUD/query/SSE tests against localhost
- GitHub Actions jobs for unit/mock tests on every push
- optional emulator job for release validation

## Status

The `0.2.0` typed collection/query API and local emulator stress suite are
implemented. Query/stream adapters are available; the independent upstream
`rtdb-rs` namespace release remains tracked in the first-class companion plan.

## License

MIT
