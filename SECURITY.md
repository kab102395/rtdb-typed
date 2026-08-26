# Security policy

Please do not report credentials, access tokens, private Firebase URLs, or
other sensitive data in a public issue. Send security reports privately to the
repository owner with reproduction steps, impact, and a proposed mitigation.

This crate does not store credentials or implement authentication. It delegates
authentication and transport to `rtdb-rs`; reports involving those boundaries
should identify the affected dependency and version.
