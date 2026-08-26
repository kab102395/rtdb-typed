# Testing rtdb-typed without a production Firebase database

The default rule for this project is: **tests must not require a billable Firebase Realtime Database instance.**

## Test layers

### Layer 1: pure unit tests

Run:

```bash
cargo test
```

These tests cover Serde conversion, optional/null handling, typed error conversion, and other logic that does not require HTTP.

### Layer 2: local HTTP contract tests

The preferred integration-test layer will use a local mock HTTP server. It will return Firebase-shaped JSON responses and assert the requests emitted through `rtdb-rs`.

This layer should cover:

- typed GET response decoding
- typed PUT/PATCH request serialization and response decoding
- POST push-key response parsing
- DELETE behavior
- error JSON and malformed JSON
- query response decoding
- SSE event payload decoding

This gives deterministic coverage of the `rtdb-typed -> rtdb-rs -> HTTP` boundary without Firebase credentials, internet access, or billable operations.

### Layer 3: Firebase Realtime Database Emulator

Firebase provides an official Realtime Database emulator in the Local Emulator Suite. This repository contains `firebase.json`, `database.rules.json`, and `.firebaserc` configured around the demo project ID `demo-rtdb-typed`.

Using a `demo-` project ID is intentional: demo projects have no live Firebase resources, so an accidental request to an un-emulated Firebase product fails instead of reaching production. The checked-in runner also prints Docker bindings and refuses to start if Docker or the host already owns ports 9000 or 4000.

Install the Firebase CLI and emulator prerequisites, then start only the database emulator:

```bash
firebase emulators:start --only database --project demo-rtdb-typed
```

Or run a command while Firebase automatically starts and stops the emulator:

```bash
firebase emulators:exec --only database --project demo-rtdb-typed "cargo test --test emulator"
```

The repeatable local command is:

```bash
./scripts/test-emulator.sh
```

That command runs the smoke test and the standard stress profile: 32 concurrent
workers perform 50 typed PUT/GET/PATCH/GET/POST sequences each, plus typed
key-range and limit queries. The test verifies typed responses, collection
cardinality, elapsed time, and cleanup. The 64-worker/100-sequence profile is
available by invoking `emulator_stress_high_profile_manual` directly. These are
functional concurrency stress tests, not production capacity benchmarks; they
do not claim latency or maximum-throughput limits.

The Realtime Database emulator defaults to `127.0.0.1:9000`; the Emulator Suite UI defaults to `127.0.0.1:4000`.

## Namespace and query behavior

The RTDB emulator REST API identifies a database instance with the `ns` query parameter, for example:

```text
http://127.0.0.1:9000/users/alice.json?ns=demo-rtdb-typed
```

The checked-in emulator runner uses the `demo-rtdb-typed` namespace and
instance-specific rules. `singleProjectMode` is disabled because the suite
also creates isolated mock-server namespaces. Firebase keys are indexed
automatically; child-field filter coverage is deterministic in the localhost
contract tests. Do not point automated tests at a real Firebase RTDB instance.

## Emulator security rules

`database.rules.json` is deliberately open because it is for the local demo emulator only:

```json
{
  "rules": {
    ".read": true,
    ".write": true
  }
}
```

Do not deploy these rules to a production Firebase project.

## Release gate

Before publishing a release:

1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test`
4. local HTTP contract tests
5. `./scripts/test-emulator.sh`
6. `cargo package --list`, `cargo package`, and `cargo publish --dry-run`
