"""Small JSON-RPC client for an isolated, installed Codex app-server."""
import asyncio
import json


class Rpc:
    def __init__(self, process):
        self.process = process
        self.sequence = 0
        self.pending = {}
        self.events = asyncio.Queue()
        self.reader = asyncio.create_task(self._read())

    @classmethod
    async def start(cls, executable, env, log_path):
        log = open(log_path, "ab")
        process = await asyncio.create_subprocess_exec(
            executable, "app-server", "--stdio", env=env,
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
            stderr=log,
        )
        log.close()
        rpc = cls(process)
        await rpc.call("initialize", {
            "clientInfo": {"name": "ctc_remote_compat_probe", "version": "1.0"},
            "capabilities": {"experimentalApi": True},
        })
        await rpc.notify("initialized", {})
        return rpc

    async def _read(self):
        while line := await self.process.stdout.readline():
            try:
                message = json.loads(line)
            except ValueError:
                continue
            if "id" in message and ("result" in message or "error" in message):
                future = self.pending.pop(message["id"], None)
                if future and not future.done():
                    future.set_result(message)
            elif "id" in message:
                # No probe permits executing tools or making approval decisions.
                await self._write({"id": message["id"], "error": {
                    "code": -32601, "message": "Probe does not handle tool requests"
                }})
            else:
                await self.events.put(message)

    async def _write(self, value):
        self.process.stdin.write((json.dumps(value) + "\n").encode())
        await self.process.stdin.drain()

    async def call(self, method, params, timeout=60):
        self.sequence += 1
        request_id = self.sequence
        future = asyncio.get_running_loop().create_future()
        self.pending[request_id] = future
        await self._write({"id": request_id, "method": method, "params": params})
        response = await asyncio.wait_for(future, timeout)
        if "error" in response:
            raise RuntimeError(f"{method}: {json.dumps(response['error'])}")
        return response.get("result")

    async def notify(self, method, params):
        await self._write({"method": method, "params": params})

    async def turn(self, thread_id, prompt, timeout=150):
        while not self.events.empty():
            self.events.get_nowait()
        result = await self.call("turn/start", {
            "threadId": thread_id,
            "input": [{"type": "text", "text": prompt, "text_elements": []}],
            "effort": "low", "serviceTierForTurn": "default",
        })
        turn_id = result["turn"]["id"]
        text = []
        deltas = 0
        errors = []
        deadline = asyncio.get_running_loop().time() + timeout
        while True:
            event = await asyncio.wait_for(
                self.events.get(), deadline - asyncio.get_running_loop().time()
            )
            method, params = event.get("method"), event.get("params", {})
            if params.get("threadId") != thread_id:
                continue
            if method == "item/agentMessage/delta":
                if params.get("turnId") not in (None, turn_id):
                    continue
                text.append(params.get("delta", ""))
                deltas += 1
            if method == "error":
                errors.append(params.get("error"))
            if method == "turn/completed" and params["turn"]["id"] == turn_id:
                return {"status": params["turn"]["status"], "text": "".join(text),
                        "delta_count": deltas,
                        "error": params["turn"].get("error"), "errors": errors}

    async def close(self):
        if self.process.returncode is None:
            self.process.stdin.close()
            try:
                await asyncio.wait_for(self.process.wait(), 8)
            except asyncio.TimeoutError:
                self.process.terminate()
                await self.process.wait()
        self.reader.cancel()
