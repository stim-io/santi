# Session Locking

## Goal

Prevent conflicting session mutations on the same session.

## Rules

- scope:
  - `session/send`
  - explicit `session/fork` against the parent session
  - explicit `session/compact`
- fail fast on contention
- do not queue
- do not silently disable protection if Redis is unavailable

## Lock key

- `lock:session_send:<session_id>`

Rule:

- `session/send`, `session/fork`, and `session/compact` share the same lock family for a given parent session id
- the lock key name is `lock:session_send:<session_id>`

## Behavior

- acquire with Redis `SET key value NX PX <ttl_ms>`
- release with compare-and-delete, not plain `DEL`
- renew while the turn is active
- let TTL recover abandoned locks

## Error semantics

- contention => busy / `409`
- Redis failure => request failure
