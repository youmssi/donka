#!/usr/bin/env python3
"""Controlled connector service for live feature tests; only synthetic bodies are retained.

POST /<case>/<mode>: score, retry, failure, timeout or non-json.
GET /calls/<case> reports received bodies and whether authentication matched, never the key.
"""
import json
import os
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

KEY = os.environ['FAKE_BUREAU_KEY']
CALLS = {}
LOCK = threading.Lock()


class Bureau(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/health':
            return self.answer(200, {'ready': True})
        if not self.path.startswith('/calls/'):
            return self.answer(404, {})
        with LOCK:
            calls = list(CALLS.get(self.path.removeprefix('/calls/'), []))
        self.answer(200, {'calls': calls})

    def do_POST(self):
        parts = self.path.strip('/').split('/')
        if len(parts) != 2 or parts[1] not in {'score', 'retry', 'failure', 'timeout', 'non-json'}:
            return self.answer(404, {})
        case, mode = parts
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))))
        authenticated = self.headers.get('X-Api-Key') == KEY
        with LOCK:
            calls = CALLS.setdefault(case, [])
            calls.append({'body': body, 'authenticated': authenticated, 'mode': mode})
            attempt = sum(call['mode'] == mode for call in calls)
        if not authenticated:
            return self.answer(401, {'error': 'unknown key'})
        if mode == 'timeout':
            time.sleep(1)
        if mode == 'failure' or (mode == 'retry' and attempt == 1):
            return self.answer(503, {'error': 'controlled failure'})
        if mode == 'non-json':
            return self.answer(200, b'not JSON')
        self.answer(200, {'score': 712 if body.get('nationalId') == 'CM-1' else 480, 'available': True})

    def answer(self, status, payload):
        data = payload if isinstance(payload, bytes) else json.dumps(payload).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            # The timeout journey intentionally closes the connection before the answer.
            pass

    def log_message(self, *_):
        pass


if __name__ == '__main__':
    ThreadingHTTPServer((sys.argv[1], int(sys.argv[2])), Bureau).serve_forever()
