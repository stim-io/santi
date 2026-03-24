# Redis Lock Plan

This file records the minimal concurrency-lock direction for `session/send`.

## Goal

Guarantee that the same session cannot run multiple concurrent send turns.

## Scope

- apply only to `session/send`
- keep the first version small and explicit
- fail fast on lock contention instead of queueing

## Placement

- put the lock in `service/session/send.rs`
- do not put lock logic in `handler/`
- do not hide lock logic inside `runtime/`

## Redis Key

- use explicit business keys rather than hiding key construction inside the lock crate
- current session-send key: `lock:session_send:<session_id>`
- recommended general shape: `lock:<resource_or_action>:<id>`
- value: unique lock token for the current request

## Acquire

- use Redis `SET key value NX PX <ttl_ms>`
- if acquire fails, treat the session as busy

## Release

- do not use plain `DEL`
- release through compare-and-delete Lua so only the current holder can delete the lock

## TTL And Recovery

- initial TTL: `120s`
- renew periodically while the turn is active
- if the process dies, let TTL expiry recover the lock automatically

## Error Semantics

- lock contention should return a clear busy error
- Redis unavailability should fail the request rather than silently disabling protection

## Observability

Log at least:

- lock acquire start/success/fail
- lock renew success/fail
- lock release success/fail

Required fields:

- `session_id`
- `request_id` or lock token
- `ttl_ms`

## Local Development

The local `santi/docker-compose.yml` should eventually add:

- `redis` service
- `REDIS_URL=redis://redis:6379/0` for `santi`

Suggested host port:

- `16379:6379`

## Crate Shape

The planned lock crate should stay very small.

Preferred API shape:

```rust
lock_client
    .with_lock("lock:session_send:<session_id>", async || {
        // protected critical section
    })
    .await
```

Guidelines:

- business code builds the key explicitly
- the crate owns acquire, renew, and safe release
- the first version should not add queueing, fairness, or multi-backend abstractions
