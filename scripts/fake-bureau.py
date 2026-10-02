#!/usr/bin/env python3
"""A credit bureau for the end-to-end test: POST /score answers a score when the
X-Api-Key header holds FAKE_BUREAU_KEY, 401 otherwise.

    FAKE_BUREAU_KEY=... scripts/fake-bureau.py 127.0.0.1 8099
"""
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

KEY = os.environ["FAKE_BUREAU_KEY"]
SCORES = {"CM-1": 712}


class Bureau(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        if self.path != "/score":
            return self.answer(404, {})
        if self.headers.get("X-Api-Key") != KEY:
            return self.answer(401, {"error": "unknown key"})
        return self.answer(200, {"score": SCORES.get(body.get("nationalId"), 480), "available": True})

    def answer(self, status, payload):
        data = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_):
        pass


if __name__ == "__main__":
    ThreadingHTTPServer((sys.argv[1], int(sys.argv[2])), Bureau).serve_forever()
