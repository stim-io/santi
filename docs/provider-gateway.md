# Provider Gateway

## Split

Keep the gateway thin. Keep provider semantics in `santi-provider`.

### Gateway owns

- upstream auth
- token/header injection
- account routing and health
- basic forwarding

### `santi-provider` owns

- `/responses` request shaping
- `instructions`, `input`, `tools`, `previous_response_id`
- SSE parsing
- response mapping
- tool-call extraction

## Rule

The gateway decides which upstream account sends the request.

`santi-provider` decides what the request means.
