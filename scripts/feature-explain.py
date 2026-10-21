#!/usr/bin/env python3
"""Controlled explanation endpoint for disposable live feature tests."""

import json
import os
import re
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CASES = {}
LOCK = threading.Lock()
MODES = {"success", "unavailable", "declined", "unreadable"}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        # Request bodies may hold the synthetic privacy probes; never print them.
        pass

    def reply(self, status, body):
        data = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def body(self):
        size = int(self.headers.get("Content-Length", "0"))
        if not 0 < size <= 1024 * 1024:
            raise ValueError("invalid body size")
        return json.loads(self.rfile.read(size))

    def do_GET(self):
        if self.path == "/health":
            return self.reply(200, {"ready": True})
        if self.path.startswith("/cases/"):
            with LOCK:
                case = CASES.get(self.path.removeprefix("/cases/"), {"calls": []})
                return self.reply(200, case)
        self.reply(404, {"error": "not found"})

    def do_PUT(self):
        try:
            body = self.body()
            if not self.path.startswith("/cases/") or body.get("mode") not in MODES:
                return self.reply(400, {"error": "invalid case"})
            with LOCK:
                case = CASES.setdefault(self.path.removeprefix("/cases/"), {"calls": []})
                case["mode"] = body["mode"]
            self.reply(200, {"ready": True})
        except (ValueError, TypeError):
            self.reply(400, {"error": "invalid JSON"})

    def do_POST(self):
        if self.path not in ("/v1/messages", "/v1/chat/completions"):
            return self.reply(404, {"error": "not found"})
        try:
            body = self.body()
            prompt = body["messages"][-1]["content"]
            match = re.search(r"<decision>\s*(.*?)\s*</decision>", prompt, re.S)
            decision = json.loads(match.group(1))
            key = decision["input"]["caseId"]
            anthropic = self.path == "/v1/messages"
            expected = os.environ["FEATURE_EXPLAIN_KEY"]
            authenticated = (
                self.headers.get("x-api-key") == expected
                if anthropic
                else self.headers.get("Authorization") == f"Bearer {expected}"
            )
            with LOCK:
                case = CASES.setdefault(key, {"calls": [], "mode": "success"})
                case["calls"].append({"body": body, "authenticated": authenticated, "path": self.path})
                mode = case["mode"]
            if not authenticated:
                return self.reply(401, {"error": "invalid fixture credential"})
            if mode == "unavailable":
                return self.reply(503, {"error": "controlled provider outage"})
            if mode == "unreadable":
                return self.reply(200, {"unexpected": "controlled malformed answer"})
            french = prompt.endswith("French.")
            text = (
                "Le score du service est 712. La règle a conservé ce résultat."
                if french
                else "The service score is 712. The rule preserved that result."
            )
            if anthropic:
                return self.reply(200, {
                    "model": "feature-explain-responder",
                    "stop_reason": "refusal" if mode == "declined" else "end_turn",
                    "content": [{"type": "text", "text": text}],
                })
            self.reply(200, {
                "model": "feature-explain-responder",
                "choices": [{
                    "finish_reason": "content_filter" if mode == "declined" else "stop",
                    "message": {"content": text},
                }],
            })
        except (ValueError, TypeError, KeyError, IndexError, AttributeError):
            self.reply(400, {"error": "invalid explanation request"})


if __name__ == "__main__":
    if not os.environ.get("FEATURE_EXPLAIN_KEY"):
        raise SystemExit("Set FEATURE_EXPLAIN_KEY for the controlled endpoint")
    ThreadingHTTPServer((sys.argv[1], int(sys.argv[2])), Handler).serve_forever()
