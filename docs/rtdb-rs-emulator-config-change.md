# rtdb-rs emulator/config change plan

## Purpose

`rtdb-typed` now has local Firebase Realtime Database emulator coverage, but `rtdb-rs 0.3.1` owns request URL construction and does not yet provide a clean general mechanism for preserving Firebase emulator namespace/configuration query parameters.

This should be fixed in `rtdb-rs` itself, not worked around in `rtdb-typed`.

The goal is to make local emulator testing a first-class transport capability while keeping the design general enough for future Firebase REST configuration needs.

## Current problem

Production Firebase URLs normally encode the database identity in the host:

```text
https://<project>-default-rtdb.firebaseio.com/path.json
```

The local RTDB emulator is commonly addressed through localhost and uses an explicit namespace/project identifier such as:

```text
http://127.0.0.1:9000/path.json?ns=demo-rtdb-typed
```

`rtdb-rs` currently constructs its own query string for authentication and Firebase query options. A caller cannot safely attach persistent base query parameters such as `ns` without relying on string tricks that can be lost or malformed by later URL construction.

## Design requirement

Do not add a one-off hardcoded test hack.

The upstream API should support persistent request-level/base configuration in a general way.

Possible designs include one of the following.

### Option A: client configuration/builder

Conceptual shape:

```text
RtdbClientConfig
  base_url
  token
  namespace: Option<String>
  base_query_params
```

and:

```text
RtdbClient::builder()
  .base_url(...)
  .token(...)
  .namespace("demo-rtdb-typed")
  .build()
```

### Option B: explicit namespace constructor/helper

Conceptual shape:

```text
RtdbClient::new(...)
RtdbClient::with_namespace(...)
```

This is simpler but less extensible.

### Option C: preserve query params already present in base URL

Allow:

```text
http://127.0.0.1:9000?ns=demo-rtdb-typed
```

and merge those base query parameters into every generated request.

This can be useful, but URL parsing/merging must be done structurally rather than by concatenating strings.

## Recommended direction

Prefer a real URL/configuration abstraction rather than raw string concatenation.

The implementation should ideally:

- parse and normalize the base URL once;
- preserve existing base query parameters;
- allow an explicit optional Firebase namespace;
- merge auth parameters safely;
- merge query-builder parameters safely;
- percent-encode values exactly once;
- avoid duplicate `?` or `&` construction;
- preserve current production behavior by default;
- keep existing constructors source-compatible where practical.

A small `RtdbClientBuilder` is preferable if it can be introduced without unnecessary API churn.

## Scope in rtdb-rs

Every request path must use the same URL construction rules:

- `get`;
- `put`;
- `patch`;
- `post`;
- `delete`;
- normal queries;
- shallow queries;
- SSE streams;
- filtered SSE streams;
- public `build_url()` debugging.

Do not fix only CRUD while leaving query/stream paths inconsistent.

## Authentication interaction

Current token behavior must remain intact:

- OAuth2 access-token style uses `access_token`;
- Firebase ID token/other supported style uses `auth`.

Namespace/config parameters must coexist with authentication and Firebase query parameters.

Example final request shapes should be valid regardless of parameter ordering:

```text
/path.json?ns=demo-rtdb-typed&auth=token
```

or:

```text
/path.json?ns=demo-rtdb-typed&access_token=token&orderBy=...
```

Tests should not require one arbitrary ordering unless the API explicitly guarantees it.

## Unit and contract testing

Add upstream tests covering:

- production base URL with no namespace;
- localhost base URL with namespace;
- base URL already containing a query parameter;
- OAuth `access_token` plus namespace;
- `auth` plus namespace;
- namespace plus `orderBy`;
- namespace plus equality/range filters;
- namespace plus shallow query;
- namespace plus stream URL;
- encoded namespace/query values where applicable;
- base URLs with trailing slash;
- child paths with leading/trailing slash;
- no double encoding;
- no malformed `??`, `&&`, or lost parameters.

## Local emulator integration suite

Add an optional/ignored emulator integration suite directly to `rtdb-rs`, separate from `rtdb-typed`.

Use only a `demo-` project ID and localhost.

Required coverage:

- raw JSON PUT/GET/PATCH/POST/DELETE;
- namespace isolation between two demo namespaces if supported by the emulator invocation;
- filtered query;
- shallow query;
- initial SSE PUT;
- subsequent PUT event;
- PATCH event;
- delete as null PUT;
- child-path stream;
- filtered stream;
- cleanup.

## rtdb-rs emulator stress testing

The transport crate should get its own stress harness so failures can be attributed to transport/SSE behavior before `rtdb-typed` adds Serde conversion on top.

### CRUD/query stress profile

Standard profile:

```text
32 workers
50 mixed operation sequences per worker
```

Each worker performs:

```text
PUT JSON
GET and verify JSON
PATCH
GET and verify
POST and verify push key
filtered query
range/limit query
```

Use run-specific roots and deterministic assertions.

### SSE stress profile

Standard profile:

```text
24 concurrent streams
40 mutations per stream root
```

Verify:

- initial event received;
- expected PUT/PATCH events received;
- deletion semantics;
- no parser failures;
- streams terminate cleanly.

### Fan-out profile

Manual/ignored heavier test:

```text
32 subscribers to one path
100 mutations
```

Verify subscriber event counts and payload progression.

### Heavy manual profile

Optional development profile:

```text
64 workers/streams
100 operation or mutation sequences each
```

Record diagnostics but do not publish emulator results as production performance claims.

## Safety controls

The local test runner should:

- require a project ID beginning with `demo-`;
- bind emulator traffic to localhost;
- refuse known production Firebase URLs in emulator test configuration;
- use run-specific database roots;
- clean up test data;
- keep emulator tests ignored or explicitly invoked so normal `cargo test` never depends on Firebase CLI/Node.

## Downstream acceptance criteria

The upstream change is complete when `rtdb-typed` can configure one client for:

```text
http://127.0.0.1:9000
namespace = demo-rtdb-typed
```

and successfully run typed CRUD, typed queries, and typed SSE tests without string manipulation or a production Firebase resource.

`rtdb-sync` must also be able to reuse the same configuration path later.

## Suggested release

Ship this as the next compatible `rtdb-rs` patch/minor release appropriate to the final API surface, then update `rtdb-typed` to depend on that release before declaring its emulator-backed realtime testing complete.
