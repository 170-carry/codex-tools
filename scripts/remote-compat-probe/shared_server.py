"""Verify two clients sharing one authenticated local server, without Remote pairing."""
import asyncio
import json
import os
from pathlib import Path
import secrets
import signal

import websockets

from environment import CODEX, PROXY, configure, free_port, hashes, private_write
from observer import start_observer
from rpc import Rpc


class WsRpc(Rpc):
    def __init__(self, connection):
        self.connection = connection
        super().__init__(None)

    @classmethod
    async def connect(cls, port, token):
        connection = await websockets.connect(
            f"ws://127.0.0.1:{port}", extra_headers={"Authorization": "Bearer " + token},
            max_size=16 * 1024 * 1024,
        )
        client = cls(connection)
        await client.call("initialize", {
            "clientInfo": {"name": "ctc_remote_shared_probe", "version": "1.0"},
            "capabilities": {"experimentalApi": True},
        })
        await client.notify("initialized", {})
        return client

    async def _write(self, value):
        await self.connection.send(json.dumps(value))

    async def _read(self):
        async for raw in self.connection:
            message = json.loads(raw)
            future = self.pending.pop(message.get("id"), None)
            if future and not future.done():
                future.set_result(message)
            elif "method" in message:
                await self.events.put(message)

    async def close(self):
        await self.connection.close()
        self.reader.cancel()


async def main():
    root = Path(Path('/tmp/ctc-229-current').read_text().strip())
    report = json.loads((root / "report.json").read_text())
    baseline = hashes(Path.home() / ".codex")
    home = root / "codex-home"
    key = (root / "pool/api-proxy.key").read_text()
    token = json.loads((home / "auth.json").read_text())["tokens"]["access_token"]
    env = dict(os.environ, CODEX_HOME=str(home))
    env["NO_PROXY"] = env["no_proxy"] = "127.0.0.1,localhost,::1"
    for name in ["OPENAI_API_KEY", "OPENAI_BASE_URL"]:
        env.pop(name, None)
    port, proxy_port, observer_port = free_port(), free_port(), free_port()
    configure({"home": home, "key": key, "observer_port": observer_port}, True)
    ws_token = secrets.token_urlsafe(32)
    private_write(root / "ws-token", ws_token)
    log = open(root / "shared-private.log", "ab")
    proxy = await asyncio.create_subprocess_exec(
        PROXY, "serve", "--host", "127.0.0.1", "--port", str(proxy_port),
        "--data-dir", str(root / "pool"), "--no-sync-current-auth",
        env=env, stdout=log, stderr=log,
    )
    observer = start_observer(observer_port, proxy_port, token, key, root / "requests.jsonl")
    server = await asyncio.create_subprocess_exec(
        CODEX, "app-server", "--listen", f"ws://127.0.0.1:{port}",
        "--ws-auth", "capability-token", "--ws-token-file", str(root / "ws-token"),
        env=env, stdout=log, stderr=log,
    )
    clients = []
    checks = {}
    request_log = root / "requests.jsonl"
    prior_requests = len(request_log.read_text().splitlines())

    def record(name, value):
        checks[name] = value
        print(name, json.dumps(value), flush=True)

    try:
        for _ in range(100):
            try:
                one = await WsRpc.connect(port, ws_token)
                clients.append(one)
                break
            except OSError:
                await asyncio.sleep(0.1)
        else:
            raise TimeoutError("Shared server did not start")
        two = await WsRpc.connect(port, ws_token)
        clients.append(two)
        thread_id = report["test_threads"]["proxy"]
        resumed = await one.call("thread/resume", {"threadId": thread_id,
                                                   "modelProvider": "ctc_remote_probe"})
        record("resumed_provider", {"provider": resumed.get("modelProvider")})
        turn = asyncio.create_task(one.turn(thread_id, "Reply with the saved marker, followed by the numbers 1 to 20."))
        # Wait for an actual active turn; do not infer progress from elapsed time.
        for _ in range(100):
            read = await two.call("thread/read", {"threadId": thread_id, "includeTurns": True})
            turns = read["thread"].get("turns", [])
            if turns and turns[-1].get("status") == "inProgress":
                break
            await asyncio.sleep(0.1)
        record("second_client_read_during_turn", {
            "turn_status": turns[-1].get("status") if turns else None,
            "turn_count": len(turns), "thread_matches": read["thread"]["id"] == thread_id,
        })
        await two.call("thread/resume", {"threadId": thread_id,
                                         "modelProvider": "ctc_remote_probe"})
        record("second_client_resume_same_server", {"accepted": True})
        record("first_client_turn", await turn)
        streamed = [event for event in list(two.events._queue)
                    if event.get("method") == "item/agentMessage/delta"]
        record("second_client_live_stream", {"delta_count": len(streamed)})
        record("second_client_continue", await two.turn(thread_id, "Reply only with the saved marker."))
        requests = [json.loads(line) for line in request_log.read_text().splitlines()[prior_requests:]]
        record("observed_proxy_requests", {
            "count": len(requests),
            "all_successful": bool(requests) and all(r["status"] == 200 for r in requests),
            "chatgpt_auth_preserved": bool(requests) and all(r["chatgpt_authorization_preserved"] for r in requests),
            "separate_proxy_key_present": bool(requests) and all(r["separate_proxy_key_present"] for r in requests),
        })
    except Exception as error:
        message = str(error).replace(key, "[KEY]").replace(token, "[TOKEN]").replace(ws_token, "[WS_TOKEN]")
        record("error", {"type": type(error).__name__, "message": message})
    finally:
        for client in clients:
            await client.close()
        for process in [server, proxy]:
            if process.returncode is None:
                process.send_signal(signal.SIGINT)
                try:
                    await asyncio.wait_for(process.wait(), 10)
                except asyncio.TimeoutError:
                    process.kill()
                    await process.wait()
        observer.shutdown()
        log.close()
        record("live_files_unchanged", baseline == hashes(Path.home() / ".codex"))
        report["shared_server_checks"] = checks
        (root / "report.json").write_text(json.dumps(report, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
