# Provider Gateway

This file records the stable boundary between upstream auth/gateway concerns and `santi-provider` request shaping.

## Goal

Keep upstream account/auth management thin and operationally friendly, while keeping provider protocol ownership inside `santi-provider`.

This matters for two reasons:

- `santi-provider` should remain the real owner of OpenAI `/responses` request construction and response mapping
- upstream account routing should remain easy to evolve into a managed account pool without distorting runtime/provider contracts

## Split

Use this split:

- thin auth proxy / gateway
- `santi-provider/openai-compatible`

### Thin Auth Proxy / Gateway

The proxy should own only upstream account-facing concerns.

Keep here:

- OAuth login, token storage, and refresh
- auth header injection
- `chatgpt-account-id` or equivalent upstream account headers
- basic request forwarding and header hygiene
- optional `/models` passthrough or aggregation
- account health tracking, rate limiting, and future account-pool routing

Do not keep here:

- `previous_response_id` rewriting
- response cache for continuation semantics
- chat/responses conversion
- tool-call extraction
- SSE semantic parsing
- prompt cache key generation

In short:

- the proxy answers "which upstream account should send this request, and with what auth?"
- the proxy should not answer "what should the request mean?"

### `santi-provider/openai-compatible`

`santi-provider` should own provider protocol semantics.

Keep here:

- `/responses` request shaping
- `instructions`, `input`, `tools`, `function_call_outputs`, and `previous_response_id`
- `prompt_cache_key`
- SSE parsing
- `response_id` tracking
- `function_call` extraction
- `ProviderEvent` mapping

In short:

- `santi-provider` answers "what request do we want to make, and how do we interpret the response?"

## Minimal Interface

The preferred gateway interface is intentionally small:

- `POST /openai/v1/responses`
- optional `GET /v1/models`

The proxy should accept OpenAI Responses-style JSON and forward it as directly as possible.

The proxy should not become a second provider adapter.

## Account Pool Direction

This split is the preferred foundation for future account-pool management.

Why:

- account pools are an auth/routing problem, not a runtime orchestration problem
- provider request semantics should stay stable even if account selection changes
- `santi-provider` should not need to know which concrete account served the request

Preferred future shape:

- `santi-provider` sends a normal `/openai/v1/responses` request
- the gateway leases an account from the pool
- the gateway injects upstream auth and forwards the request
- the gateway updates account health, cooldown, or failure state afterward

This keeps account-pool evolution mostly inside the gateway.

## Current Guidance

- shrink the OpenAI provider gateway toward auth proxy behavior
- continue growing OpenAI request/response ownership inside `santi-provider/openai-compatible`
- avoid moving provider semantics into the gateway just because auth already lives there
- treat account-pool support as a gateway concern unless it clearly changes provider-facing protocol semantics
