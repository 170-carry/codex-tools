#!/usr/bin/env python3
"""Exercise the real Codex picker with dummy auth, loopback HTTP, and temp storage.

No inference requests or user credentials are used. Any client SQLite files are
created in a fresh temporary directory; this never queries the user's history.
"""
import argparse
import copy
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


ROOT = Path(__file__).resolve().parents[1]
SEED = ROOT / "src-tauri/src/proxy_service/catalog/codex-models.json"


def probe(binary, base_url, catalog, cache=None):
    with tempfile.TemporaryDirectory(prefix="codex-tools-model-picker-") as directory:
        home = Path(directory)
        config = (
            f"openai_base_url = {json.dumps(base_url)}\n"
            'model_provider = "openai"\ncli_auth_credentials_store = "file"\n'
        )
        if catalog is not None:
            catalog_path = home / "codex-tools-models.json"
            catalog_path.write_text(json.dumps(catalog), encoding="utf-8")
            config += f"model_catalog_json = {json.dumps(str(catalog_path))}\n"
        config += "[analytics]\nenabled = false\n"
        (home / "config.toml").write_text(config, encoding="utf-8")
        (home / "auth.json").write_text(
            json.dumps({"auth_mode": "apikey", "OPENAI_API_KEY": "sk-local-model-picker-test"}),
            encoding="utf-8",
        )
        if cache is not None:
            (home / "models_cache.json").write_text(json.dumps(cache), encoding="utf-8")
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(("CODEX_", "OPENAI_"))}
        # Scope the documented client home setting to this child process only.
        env["CODEX_HOME"] = directory
        with (home / "stderr.log").open("w", encoding="utf-8") as stderr:
            process = subprocess.Popen(
                [binary, "app-server", "--listen", "stdio://"],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr,
                text=True, env=env, cwd=directory,
            )
            messages = queue.Queue()

            def read_messages():
                for line in process.stdout:
                    messages.put(json.loads(line))
                messages.put(None)

            reader = threading.Thread(target=read_messages, daemon=True)
            reader.start()

            def send(message):
                process.stdin.write(json.dumps(message) + "\n")
                process.stdin.flush()

            def rpc(request_id, method, params):
                send({"id": request_id, "method": method, "params": params})
                deadline = time.monotonic() + 20
                while time.monotonic() < deadline:
                    message = messages.get(timeout=max(.01, deadline - time.monotonic()))
                    if message is None:
                        raise RuntimeError((home / "stderr.log").read_text())
                    if message.get("id") == request_id:
                        if "error" in message:
                            raise RuntimeError(json.dumps(message["error"]))
                        return message["result"]
                raise TimeoutError(method)

            try:
                rpc(1, "initialize", {"clientInfo": {
                    "name": "codex_tools_model_picker_test", "title": "Model picker test", "version": "1.0",
                }})
                send({"method": "initialized"})
                models = rpc(2, "model/list", {"includeHidden": False})["data"]
                return models
            finally:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                reader.join(timeout=1)
                process.stdin.close()
                process.stdout.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", default=shutil.which("codex"))
    parser.add_argument("--output", type=Path, default=Path("/tmp/codex-tools-issue219/model-discovery.json"))
    args = parser.parse_args()
    if not args.codex:
        parser.error("Supply --codex with the desktop-bundled Codex binary")
    seed = json.loads(SEED.read_text(encoding="utf-8"))
    for model in seed["models"]:
        model["base_instructions"] = model["model_messages"]["instructions_template"]
    requests = []

    class ModelsServer(BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"object": "list", "data": [
                {"id": model["slug"], "object": "model", "owned_by": "openai", "created": 0}
                for model in seed["models"]
            ]}).encode())

        def log_message(self, *args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), ModelsServer)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base_url = f"http://127.0.0.1:{server.server_port}/v1"
    old = {"models": [model for model in seed["models"] if model["slug"] == "gpt-6-sol"]}
    empty = copy.deepcopy(old)
    empty["models"][0].update(visibility="hide", supported_in_api=False)
    cache = dict(old, fetched_at="2000-01-01T00:00:00Z", client_version="0.155.1", etag="stale-test")
    results = {}
    try:
        for name, catalog, cached in [
            ("legacy", None, None),
            ("fresh_binding", seed, None),
            ("stale_cache", seed, cache),
            ("restricted_key", old, None),
            ("all_disabled", empty, None),
        ]:
            requests.clear()
            models = probe(args.codex, base_url, catalog, cached)
            ids = [model["id"] for model in models]
            if name in ("fresh_binding", "stale_cache"):
                assert "gpt-6.1-sol" in ids, (name, ids)
                assert set(ids) == {model["slug"] for model in seed["models"]}
                sol = next(model for model in models if model["id"] == "gpt-6.1-sol")
                assert sol["defaultReasoningEffort"] == "low", sol
            elif name == "restricted_key":
                assert ids == ["gpt-6-sol"], ids
            elif name == "all_disabled":
                assert ids == [], ids
            results[name] = {"models": ids, "http_requests": requests.copy()}
            print(f"{name}: {ids}")
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=1)
    results["client_version"] = subprocess.check_output([args.codex, "--version"], text=True).strip()
    results["legacy_issue_reproduced"] = "gpt-6.1-sol" not in results["legacy"]["models"]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    print(f"Passed. Evidence: {args.output}")


if __name__ == "__main__":
    main()
