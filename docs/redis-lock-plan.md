# Redis Lock Plan

## Goal

Prevent concurrent `session/send` on the same session.

## Rules

- scope: `session/send` only
- fail fast on contention
- do not queue
- do not silently disable protection if Redis is unavailable

## Key

- `lock:session_send:<session_id>`

## Behavior

- acquire with Redis `SET key value NX PX <ttl_ms>`
- release with compare-and-delete, not plain `DEL`
- renew while the turn is active
- let TTL recover abandoned locks

## Error semantics

- contention => busy / `409`
- Redis failure => request failure
