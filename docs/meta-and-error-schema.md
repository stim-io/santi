# Meta and Error Schema

## Scope

This document defines the shared minimal success meta and error schema for:

- HTTP API responses
- standalone `santi-cli`
- 单机 and 分布式 `santi` assembly paths

It is the shared mother contract for the same runtime result shape across surfaces.

## Canonical success meta

Success responses may include a `meta` object.

Required stable fields when `meta` is present:

- `request_id`: request correlation identifier
- `api_version`: contract version used by the response

Optional additional meta fields may be included when they are useful and stable.

## When meta can be omitted

`meta` may be omitted on success when the response carries no additional transport or runtime context beyond the data itself.

This is allowed for small read responses and simple command responses.

If a response includes out-of-band tracing, compatibility, or execution details, it should include `meta`.

## Canonical error model

Errors use a minimal stable shape:

- `code`: stable machine-readable error code
- `message`: human-readable summary
- `details`: optional structured context
- `retryable`: optional boolean hint
- `request_id`: optional request correlation identifier

Do not add extra envelope layers unless a transport already requires them.

## Error taxonomy

Use stable error codes to describe the class of failure, not the transport surface.

Recommended classes:

- `validation_error`: request shape, field, or argument validation failed
- `not_found`: requested resource does not exist
- `conflict`: request conflicts with current state or another in-flight request
- `unauthorized`: authentication is missing or invalid
- `forbidden`: authenticated but not allowed
- `rate_limited`: caller must slow down
- `timeout`: operation exceeded its allowed time
- `unavailable`: dependency or service is temporarily unavailable
- `internal_error`: unexpected server-side failure

Keep the code set small and stable. Add new codes only when they represent a real new class of failure.

## HTTP mapping rules

- Successful responses use HTTP 2xx.
- Error responses use the closest appropriate HTTP status code.
- Preserve `request_id` in the response when available.
- Preserve `code` and `message` across HTTP and non-HTTP surfaces.
- `POST /api/v1/sessions/{id}/send` concurrent use on the same session is a conflict and maps to `409 Conflict`.

Mapping guidance:

- `validation_error` -> `400 Bad Request` or `422 Unprocessable Entity` when the route already accepted the request shape but validation failed at a semantic level
- `not_found` -> `404 Not Found`
- `conflict` -> `409 Conflict`
- `unauthorized` -> `401 Unauthorized`
- `forbidden` -> `403 Forbidden`
- `rate_limited` -> `429 Too Many Requests`
- `timeout` -> `504 Gateway Timeout` or `408 Request Timeout` depending on where the timeout occurs
- `unavailable` -> `503 Service Unavailable`
- `internal_error` -> `500 Internal Server Error`

## Validation error shape

Validation errors should use `code: validation_error` and place field-level details in `details`.

Recommended `details` shape:

- `field`: field or path that failed validation
- `reason`: short machine-readable reason
- `expected`: optional expected constraint or type
- `received`: optional received value summary

Multiple validation problems may be returned together if the transport already supports it, but the shape should stay minimal and readable.

## CLI mapping rules

The standalone CLI must align with the HTTP contract semantics.

- Normal human-readable success output goes to stdout.
- Errors go to stderr.
- Exit code `0` means success.
- Non-zero exit codes mean failure.
- When `--json` is enabled, success and error output should use the same canonical result shape as HTTP where practical.
- In `--json` error output, include at least `code`, `message`, and `request_id` when available.
- CLI exit codes should reflect the error class, but must not invent new semantic classes that diverge from the shared error taxonomy.
- A CLI conflict must map to the same `conflict` class as HTTP and should exit non-zero.

## Topology mapping

单机 and 分布式 must not invent separate error semantics.

- Use the same canonical success meta and error model as HTTP and CLI.
- Map internal failures to the shared error taxonomy before surface-specific rendering.
- If a non-HTTP internal path is used during assembly or tests, it still reports the same `code`, `message`, and optional `details` fields.
- 单机 conflict handling for the same session must still be `conflict`.

## Compatibility rules

- Additive fields are allowed.
- Removing or renaming stable fields is a breaking change.
- Surfaces may omit optional fields when they are not available.
- Error and success shapes must remain consistent across HTTP, CLI, 单机, and 分布式 assembly paths.
- Transport-specific wrappers must not change the meaning of `code`, `message`, or `request_id`.

## Minimal examples

### Success

```json
{
  "meta": {
    "request_id": "req_123",
    "api_version": "v1"
  },
  "data": {}
}
```

### Error

```json
{
  "error": {
    "code": "conflict",
    "message": "session is busy",
    "request_id": "req_123",
    "retryable": false
  }
}
```

### Validation error

```json
{
  "error": {
    "code": "validation_error",
    "message": "invalid request",
    "details": {
      "field": "session_id",
      "reason": "missing"
    }
  }
}
```
