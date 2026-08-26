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
- `TypedClient::query<T>().send()` for typed query results
- `TypedClient::query<T>().stream()` for typed SSE events

`TypedEvent::Put` contains a complete typed `T`. `TypedEvent::Patch` contains
raw `serde_json::Value` because Firebase patch payloads are partial updates and
cannot safely be deserialized as a complete model.

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

### Phase 2 — typed queries (implemented)

- typed wrapper around `rtdb_rs::GetBuilder`
- `send<T>()`
- collection helpers for Firebase key/value maps
- query error tests

### Phase 3 — typed realtime streams (implemented locally)

- map `RtdbEvent::Put` and `RtdbEvent::Patch` JSON payloads into typed values
- preserve `KeepAlive` and `Cancel`
- test SSE parsing through a local mock server

### Phase 4 — emulator verification and CI

- Firebase Emulator configuration
- end-to-end CRUD/query/SSE tests against localhost
- GitHub Actions jobs for unit/mock tests on every push
- optional emulator job for release validation

## Status

The `0.1.0` foundation is implemented. Query/stream adapters are available;
the remaining compatibility work is teaching `rtdb-rs` to preserve the
emulator `ns` query parameter for fully namespace-explicit emulator tests.

## License

MIT
