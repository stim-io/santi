#!/usr/bin/env python3
import argparse
import json
import os
import sys
import urllib.error
import urllib.request


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Send one canonical session/send turn and render streamed text cleanly."
    )
    parser.add_argument("session_id", nargs="?", help="target session id")
    parser.add_argument(
        "--create",
        action="store_true",
        help="create a new session and print its id",
    )
    parser.add_argument(
        "--raw",
        action="store_true",
        help="print raw SSE data payloads instead of collapsing text deltas",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    base_url = os.environ.get("SANTI_BASE_URL", "http://127.0.0.1:18081")

    if args.create:
        if args.session_id is not None:
            print("session_id should not be provided with --create", file=sys.stderr)
            return 1
        return create_session(base_url)

    if not args.session_id:
        print("usage: printf 'hello' | ./scripts/dev/send.py <session_id>", file=sys.stderr)
        return 1

    content = sys.stdin.read()

    if not content:
        print("expected stdin content", file=sys.stderr)
        return 1

    body = json.dumps(
        {"content": [{"type": "text", "text": content}]}, ensure_ascii=False
    ).encode()

    request = urllib.request.Request(
        f"{base_url}/api/v1/sessions/{args.session_id}/send",
        method="POST",
        headers={"Content-Type": "application/json"},
        data=body,
    )

    try:
        with urllib.request.urlopen(request) as response:
            return render_stream(response, raw=args.raw)
    except urllib.error.HTTPError as exc:
        error_body = exc.read().decode("utf-8", errors="replace")
        print(error_body or f"HTTP {exc.code}", file=sys.stderr)
        return 1
    except urllib.error.URLError as exc:
        print(f"request failed: {exc}", file=sys.stderr)
        return 1


def create_session(base_url: str) -> int:
    request = urllib.request.Request(
        f"{base_url}/api/v1/sessions",
        method="POST",
    )

    try:
        with urllib.request.urlopen(request) as response:
            body = json.loads(response.read().decode("utf-8", errors="replace"))
    except urllib.error.HTTPError as exc:
        error_body = exc.read().decode("utf-8", errors="replace")
        print(error_body or f"HTTP {exc.code}", file=sys.stderr)
        return 1
    except urllib.error.URLError as exc:
        print(f"request failed: {exc}", file=sys.stderr)
        return 1

    session_id = body.get("id")
    if not session_id:
        print("missing session id in create response", file=sys.stderr)
        return 1

    print(session_id)
    return 0


def render_stream(response, raw: bool) -> int:
    saw_text = False
    fallback_output = None

    for raw_line in response:
        line = raw_line.decode("utf-8", errors="replace").strip()
        if not line.startswith("data: "):
            continue

        payload = line[6:]
        if payload == "[DONE]":
            continue

        if raw:
            print(payload)
            continue

        try:
            event = json.loads(payload)
        except json.JSONDecodeError:
            print(payload, file=sys.stderr)
            return 1

        event_type = event.get("type")
        if event_type == "response.output_text.delta":
            delta = event.get("delta", "")
            if delta:
                sys.stdout.write(delta)
                sys.stdout.flush()
                saw_text = True
        elif event_type == "response.completed":
            fallback_output = (
                event.get("response", {}).get("output_text")
                if isinstance(event.get("response"), dict)
                else None
            )
        elif event_type == "error":
            print(json.dumps(event, ensure_ascii=False), file=sys.stderr)
            return 1

    if not raw and not saw_text and fallback_output:
        sys.stdout.write(fallback_output)
        saw_text = True

    if saw_text:
        sys.stdout.write("\n")
        sys.stdout.flush()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
