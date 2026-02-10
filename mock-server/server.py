#!/usr/bin/env python3
"""Async mock HTTP server for testing dhanush load tester.

Uses only stdlib asyncio -- no pip dependencies required.
Handles high concurrency without broken pipes or thread exhaustion.
"""

import argparse
import asyncio
import json
import random
import signal
import sys
from urllib.parse import urlparse

request_count = 0


def build_response(status, body, content_type="application/json"):
    """Build a raw HTTP/1.1 response."""
    global request_count
    request_count += 1

    if isinstance(body, str):
        body = body.encode()

    reason = {
        200: "OK", 201: "Created", 204: "No Content",
        400: "Bad Request", 401: "Unauthorized", 404: "Not Found",
        500: "Internal Server Error",
    }.get(status, "OK")

    header = (
        f"HTTP/1.1 {status} {reason}\r\n"
        f"Content-Type: {content_type}\r\n"
        f"Content-Length: {len(body)}\r\n"
        f"Connection: keep-alive\r\n"
        f"\r\n"
    )
    return header.encode() + body


# Pre-build static responses for hot paths
HEALTH_RESP = build_response(200, json.dumps({"status": "ok"}))
request_count = 0  # reset after pre-build

USERS_DATA = json.dumps({
    "users": [
        {"id": i, "name": f"user_{i}", "email": f"user_{i}@example.com"}
        for i in range(1, 11)
    ]
})

LARGE_DATA = json.dumps({"data": "x" * 10000, "size": "10KB"})
CSS_DATA = "body { margin: 0; padding: 20px; font-family: sans-serif; }\n" * 20
JS_DATA = "console.log('loaded');\n" * 50
NOT_FOUND_RESP = build_response(404, json.dumps({"error": "not found"}))
request_count = 0  # reset after pre-build

FLAKY_OK = json.dumps({"status": "ok"})
FLAKY_ERR = json.dumps({"error": "random failure"})
SLOW_RESP_BODY = json.dumps({"status": "slow but ok"})


async def handle_get(path):
    if path == "/health":
        return build_response(200, json.dumps({"status": "ok"}))

    elif path == "/users":
        return build_response(200, USERS_DATA)

    elif path == "/slow":
        await asyncio.sleep(random.uniform(0.05, 0.2))
        return build_response(200, SLOW_RESP_BODY)

    elif path == "/large":
        return build_response(200, LARGE_DATA)

    elif path == "/flaky":
        if random.random() < 0.8:
            return build_response(200, FLAKY_OK)
        else:
            return build_response(500, FLAKY_ERR)

    elif path == "/status":
        return build_response(200, json.dumps({"total_requests": request_count}))

    elif path == "/style.css":
        return build_response(200, CSS_DATA, content_type="text/css")

    elif path == "/app.js":
        return build_response(200, JS_DATA, content_type="application/javascript")

    else:
        return build_response(404, json.dumps({"error": "not found"}))


async def handle_post(path, body_bytes):
    if path == "/orders":
        try:
            data = json.loads(body_bytes) if body_bytes else {}
        except (json.JSONDecodeError, UnicodeDecodeError):
            data = {}
        order = {"id": random.randint(1000, 9999), "received": data, "status": "created"}
        return build_response(201, json.dumps(order))

    elif path == "/echo":
        return build_response(200, body_bytes.decode() if body_bytes else "{}")

    elif path == "/login":
        try:
            data = json.loads(body_bytes) if body_bytes else {}
        except (json.JSONDecodeError, UnicodeDecodeError):
            return build_response(400, json.dumps({"error": "invalid json"}))
        if data.get("username") and data.get("password"):
            return build_response(200, json.dumps({"token": "mock-jwt-token-12345"}))
        else:
            return build_response(401, json.dumps({"error": "unauthorized"}))

    else:
        return build_response(404, json.dumps({"error": "not found"}))


async def handle_client(reader, writer):
    """Handle a single keep-alive connection, processing multiple requests."""
    try:
        while True:
            # Read request line
            try:
                request_line = await asyncio.wait_for(reader.readline(), timeout=30.0)
            except (asyncio.TimeoutError, ConnectionError):
                break

            if not request_line:
                break

            try:
                parts = request_line.decode().strip().split()
                if len(parts) < 2:
                    break
                method, raw_path = parts[0], parts[1]
            except (UnicodeDecodeError, ValueError):
                break

            path = urlparse(raw_path).path

            # Read headers
            content_length = 0
            while True:
                line = await reader.readline()
                if not line or line == b"\r\n":
                    break
                header = line.decode().strip().lower()
                if header.startswith("content-length:"):
                    content_length = int(header.split(":", 1)[1].strip())

            # Read body if present
            body = b""
            if content_length > 0:
                body = await reader.readexactly(content_length)

            # Route
            if method == "GET":
                response = await handle_get(path)
            elif method in ("POST", "PUT"):
                response = await handle_post(path, body)
            elif method == "DELETE":
                global request_count
                request_count += 1
                response = build_response(204, "")
            else:
                response = build_response(404, json.dumps({"error": "method not allowed"}))

            writer.write(response)
            await writer.drain()

    except (ConnectionResetError, BrokenPipeError, asyncio.IncompleteReadError):
        pass
    except Exception:
        pass
    finally:
        try:
            writer.close()
            await writer.wait_closed()
        except Exception:
            pass


async def run_server(host, port):
    server = await asyncio.start_server(handle_client, host, port, reuse_address=True)

    print(f"Mock server running on http://{host}:{port}")
    print()
    print("Available endpoints:")
    print("  GET  /health       - Health check (tiny JSON)")
    print("  GET  /users        - User list (~500B JSON)")
    print("  GET  /slow         - Slow response (50-200ms delay)")
    print("  GET  /large        - Large response (~10KB)")
    print("  GET  /flaky        - 80% success, 20% 500 errors")
    print("  GET  /status       - Request counter")
    print("  GET  /style.css    - CSS file (~600B)")
    print("  GET  /app.js       - JS file (~1KB)")
    print("  POST /orders       - Create order (accepts JSON body)")
    print("  POST /echo         - Echo back request body")
    print("  POST /login        - Mock auth (expects username/password)")
    print()
    print("Press Ctrl+C to stop")

    stop = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, stop.set)

    async with server:
        await stop.wait()

    print(f"\nShutting down. Total requests handled: {request_count}")


def main():
    parser = argparse.ArgumentParser(description="Mock HTTP server for dhanush testing")
    parser.add_argument("-p", "--port", type=int, default=8080, help="Port to listen on")
    parser.add_argument("--host", default="127.0.0.1", help="Host to bind to")
    args = parser.parse_args()

    asyncio.run(run_server(args.host, args.port))


if __name__ == "__main__":
    main()
