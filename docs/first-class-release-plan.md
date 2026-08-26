# rtdb-typed first-class release plan

## Decision

Do not publish `0.1.0` yet.

The crate is structurally sound and the current CI passes, but the public API and test surface still need a small amount of hardening before `rtdb-typed` should be presented as a first-class companion to `rtdb-rs`.

The release target is not merely "code that compiles." The target is a crate that has explicit semantics, stable-enough public types, realistic local tests, complete docs, and a reproducible release gate.

## Current strengths

- `rtdb-typed` is a thin layer over `rtdb-rs` rather than a duplicate Firebase client.
- `TypedClient` supports typed GET, optional GET, PUT, PATCH, POST push keys, and DELETE.
- `TypedQuery<T>` preserves the underlying `rtdb-rs` query builder operations.
- `TypedEvent<T>` maps the low-level realtime event stream into a typed API.
- `TypedError` preserves `rtdb-rs` failures and Serde conversion failures.
- Local HTTP contract tests exist and do not require a Firebase account.
- Firebase RTDB emulator tests are isolated behind a `demo-` project and ignored during ordinary unit/CI runs.
- CI runs format, clippy, and tests and currently passes.
- The crate has an MIT license and valid crates.io package metadata.

## Release blockers

### 1. Fix typed PATCH event semantics

Firebase SSE `patch` payloads are partial objects. A `Patch` event cannot safely promise that its payload always deserializes into the same complete `T` used by a `Put` event.

Current shape:

```text
TypedEvent<T>
  Put   { data: T }
  Patch { data: T }
```

This can fail for a normal model when Firebase sends only changed fields.

Before `0.1.0`, choose and document one explicit contract. Preferred initial contract:

```text
TypedEvent<T>
  Put   { data: T }
  Patch { data: serde_json::Value }
  KeepAlive
  Cancel
```

A later release may add an opt-in second patch type or a typed patch adapter. Do not make full-model PATCH deserialization part of the first stable public contract unless it is proven correct.

### 2. Define missing collection semantics

`get_collection<T>()` currently deserializes directly to `HashMap<String, T>`.

Decide what a missing Firebase node (`null`) means for collection helpers:

- preferred: `get_collection<T>()` returns an empty map for `null`;
- optionally add `get_optional_collection<T>()` if callers need to distinguish missing from empty;
- document malformed child behavior and Serde failures.

Add tests for empty, one-item, multi-item, malformed-item, and null collections.

### 3. Add typed query contract tests

The public query wrapper must be tested independently of the Firebase production service.

Required local HTTP tests:

- `order_by_child` + `equal_to` + limit;
- key ordering;
- numeric range;
- boolean filter;
- invalid query propagated as `TypedError::Rtdb`;
- typed collection query response;
- malformed result returns `TypedError::Serde`;
- shallow query behavior is documented and tested.

Tests should assert both the outgoing Firebase query URL and the typed result.

### 4. Add local SSE contract tests

Do not wait for a production Firebase database.

Build a localhost mock server that returns `text/event-stream` and test:

- initial `put` event;
- subsequent `put` event;
- partial `patch` event;
- `keep-alive`;
- `cancel`;
- malformed SSE JSON;
- typed deserialization failure;
- stream close behavior.

This is the main release gate for realtime behavior. The Firebase emulator is additional validation, not the only way to prove the adapter.

### 5. Resolve emulator namespace support upstream

`rtdb-rs` owns Firebase URL construction. The RTDB emulator commonly requires the `ns` query parameter when using localhost.

Add a general capability to `rtdb-rs` rather than an ad-hoc hack in `rtdb-typed`. Acceptable designs include:

- client-level persistent query parameters;
- explicit namespace configuration;
- an emulator/config constructor that still uses the same request builder.

After an upstream release containing this support, bump the `rtdb-rs` dependency and add namespace-explicit emulator tests for CRUD, queries, and SSE.

This upstream change should preserve all existing production URL behavior.

### 6. Harden the public API documentation

Every public type and public method should have rustdoc describing:

- Firebase behavior;
- null semantics;
- serialization/deserialization behavior;
- relevant errors;
- examples where useful;
- when to use `inner()` and what guarantees are lost by dropping to the low-level client.

Run:

```text
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

as a release gate.

### 7. Add copy-pasteable examples

Add an `examples/` directory with at least:

- `crud.rs` — typed model CRUD;
- `query.rs` — typed Firebase collection query;
- `stream.rs` — typed PUT events plus raw/partial PATCH semantics;
- `optional.rs` — missing-node behavior.

Examples must compile in CI. They do not need to execute against Firebase in normal CI.

### 8. Complete crates.io presentation metadata

Before publishing, make package intent explicit in `Cargo.toml`:

- explicit `readme = "README.md"`;
- explicit `documentation = "https://docs.rs/rtdb-typed"`;
- define and document MSRV with `rust-version` once tested;
- keep keywords/categories focused and valid;
- verify only intended files enter the package.

Add crates.io and docs.rs badges to the README after publication.

### 9. Add release hygiene files

Add:

- `CHANGELOG.md` using semantic-versioned release notes;
- `CONTRIBUTING.md` with local unit/mock/emulator workflow;
- `SECURITY.md` describing how to report credential/auth/security defects;
- optional `CODE_OF_CONDUCT.md` if outside contributors are expected.

The crate should make clear that emulator-open rules are test-only and must never be deployed to production.

### 10. Add package and publish dry-run gates

Before release, CI or the release checklist must successfully run:

```text
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo package --list
cargo package
cargo publish --dry-run
```

Inspect `cargo package --list` manually to ensure emulator logs, temporary state, credentials, and unrelated files are not shipped.

## First-class relationship with rtdb-rs

`rtdb-rs` remains the transport crate.

It owns:

- authentication primitives;
- Firebase REST transport;
- URL/query construction;
- raw JSON CRUD;
- raw query behavior;
- SSE parsing;
- low-level Firebase errors.

`rtdb-typed` owns:

- Serde model conversion;
- typed null/collection semantics;
- typed query results;
- typed realtime event projection;
- conversion-specific errors;
- typed ergonomics only.

Do not duplicate authentication, HTTP transport, retry policy, or Firebase URL logic in `rtdb-typed`. If typed functionality exposes a missing low-level primitive, add the primitive to `rtdb-rs` and consume it here.

## API compatibility policy

For `0.1.x`, changes may still refine the API, but avoid gratuitous churn.

Before `0.2.0`, establish:

- final event semantics for PUT/PATCH;
- final collection/null semantics;
- query builder naming and generic conventions;
- error boundaries between `rtdb-rs`, Serde, and typed convenience errors.

After those settle, treat public method names and event shapes as compatibility-sensitive.

## Test pyramid

### Layer 1 — pure unit tests

No sockets, no Firebase, no account.

Covers conversion helpers, nulls, malformed JSON, collection semantics, error conversion, and event projection helpers.

### Layer 2 — localhost protocol/contract tests

A local mock HTTP/SSE server exercises the real `rtdb-rs` dependency through `rtdb-typed`.

This should provide the majority of deterministic integration coverage.

### Layer 3 — Firebase RTDB emulator

Runs only against `demo-rtdb-typed` and localhost. Covers end-to-end Firebase-compatible REST/query/SSE behavior.

Never use a real billable project as a CI fallback.

### Layer 4 — optional manual live verification

Not required for normal development or CI. If ever used before a significant release, it must be deliberate, isolated, budget-limited, and documented. A live Firebase project must never be embedded in automated tests.

## 0.1.0 release checklist

- [ ] PATCH event semantics corrected and documented.
- [ ] Collection/null semantics finalized.
- [ ] Typed CRUD unit and localhost contract tests complete.
- [ ] Typed query localhost contract tests complete.
- [ ] Typed SSE localhost contract tests complete.
- [ ] `rtdb-rs` emulator namespace support released and dependency bumped.
- [ ] Namespace-explicit emulator CRUD test passes.
- [ ] Namespace-explicit emulator query test passes.
- [ ] Namespace-explicit emulator SSE test passes.
- [ ] Public rustdoc complete with no warnings.
- [ ] `examples/` compile in CI.
- [ ] `CHANGELOG.md` added.
- [ ] `CONTRIBUTING.md` added.
- [ ] `SECURITY.md` added.
- [ ] Cargo metadata finalized, including MSRV.
- [ ] `cargo package --list` inspected.
- [ ] `cargo package` passes.
- [ ] `cargo publish --dry-run` passes.
- [ ] CI green on the exact release commit.
- [ ] README examples match the exact `0.1.0` API.
- [ ] Tag `v0.1.0` only after crates.io publication succeeds.

## Post-0.1 roadmap

### 0.2

- richer typed collection helpers;
- optional typed patch adapters;
- typed query ergonomics that reduce explicit `HashMap<String, T>` boilerplate;
- better stream reconnection helpers only if they belong above the low-level transport layer.

### 0.3+

Evaluate only after real downstream usage:

- typed path abstractions;
- domain validation hooks;
- derive/proc-macro helpers;
- framework integrations;
- typed transactions if `rtdb-rs` first gains correct ETag/conditional-request primitives.

Do not turn `rtdb-typed` into an ORM or duplicate the Firebase SDK. Keep it a focused, dependable typed companion to `rtdb-rs`.
