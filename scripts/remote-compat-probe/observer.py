"""Loopback-only forwarding observer; never records credentials or request bodies."""
import hashlib
import http.client
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def start_observer(port, target_port, token, key, output):
    lock = threading.Lock()

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *_):
            pass

        def handle_request(self):
            body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            entry = {
                "method": self.command, "path": self.path,
                "chatgpt_authorization_preserved": self.headers.get("Authorization") == "Bearer " + token,
                "separate_proxy_key_present": self.headers.get("x-api-key") == key,
                "request_bytes": len(body),
                "request_sha256": hashlib.sha256(body).hexdigest(),
            }
            connection = http.client.HTTPConnection("127.0.0.1", target_port, timeout=150)
            headers = {k: v for k, v in self.headers.items()
                       if k.lower() not in {"host", "connection", "transfer-encoding"}}
            headers["Connection"] = "close"
            try:
                connection.request(self.command, self.path, body, headers)
                response = connection.getresponse()
                entry["status"] = response.status
                with lock, open(output, "a") as log:
                    log.write(json.dumps(entry) + "\n")
                self.send_response(response.status)
                for k, v in response.getheaders():
                    if k.lower() not in {"connection", "transfer-encoding", "content-length"}:
                        self.send_header(k, v)
                self.send_header("Connection", "close")
                self.end_headers()
                while chunk := response.read1(65536):
                    self.wfile.write(chunk)
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                pass
            finally:
                self.close_connection = True
                connection.close()

        do_GET = handle_request
        do_POST = handle_request

    server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server
