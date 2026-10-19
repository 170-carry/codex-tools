"""Run from the bundled Python 3.12; produces a credential-free report."""
import asyncio
import json
from pathlib import Path
import signal
import tempfile

from environment import CODEX, configure, hashes, prepare, start_proxy
from observer import start_observer
from rpc import Rpc


async def main():
    root = Path(tempfile.mkdtemp(prefix="ctc-229-", dir="/tmp"))
    Path('/tmp/ctc-229-current').write_text(str(root))
    state = prepare(root)
    report = {"upstream_differs_from_login": state["upstream_differs_from_login"], "checks": {}}
    checks = report["checks"]
    rpc = None
    proxy = None
    observer = None

    def record(name, value):
        checks[name] = value
        (state["root"] / "report.json").write_text(json.dumps(report, indent=2))
        print(name, json.dumps(value), flush=True)

    async def boot(mode):
        configure(state, mode == "proxy")
        return await Rpc.start(CODEX, state["env"], state["root"] / "rpc-private.log")

    async def create(client):
        return (await client.call("thread/start", {
            "cwd": str(state["root"] / "workspace"), "model": "gpt-6.1-sol",
            "sandbox": "read-only", "approvalPolicy": "never", "environments": [],
            "baseInstructions": "You are a connectivity test. Do not use any tools. Reply to the prompt in plain text.",
        }))["thread"]["id"]

    async def identity(client):
        result = await client.call("account/read", {"refreshToken": False})
        return {"type": (result.get("account") or {}).get("type"),
                "requiresOpenaiAuth": result.get("requiresOpenaiAuth")}

    try:
        proxy = await start_proxy(state)
        observer = start_observer(state["observer_port"], state["proxy_port"],
                                  state["token"], state["key"], state["root"] / "requests.jsonl")
        rpc = await boot("normal")
        record("normal_identity", await identity(rpc))
        normal_id = await create(rpc)
        record("normal_new", await rpc.turn(normal_id, "Remember the marker NORMAL229. Reply exactly NORMAL229."))
        await rpc.close()
        rpc = await boot("proxy")
        record("proxy_identity", await identity(rpc))
        record("remote_control_status", await rpc.call("remoteControl/status/read", {}))
        proxy_id = await create(rpc)
        report["test_threads"] = {"normal": normal_id, "proxy": proxy_id}
        record("proxy_new", await rpc.turn(proxy_id, "Remember the marker POOL229. Reply exactly POOL229."))
        read = await rpc.call("thread/read", {"threadId": proxy_id, "includeTurns": True})
        record("proxy_read", {"id_matches": read["thread"]["id"] == proxy_id,
                              "turn_count": len(read["thread"].get("turns", []))})
        listed = await rpc.call("thread/list", {"limit": 20, "modelProviders": []})
        record("proxy_list", {"contains_test": any(x["id"] == proxy_id for x in listed["data"])})
        default_list = await rpc.call("thread/list", {"limit": 20})
        record("proxy_default_list", {"contains_proxy": any(x["id"] == proxy_id for x in default_list["data"]),
                                      "contains_normal": any(x["id"] == normal_id for x in default_list["data"])})
        peer = await Rpc.start(CODEX, state["env"], state["root"] / "peer-private.log")
        try:
            read = await peer.call("thread/read", {"threadId": proxy_id, "includeTurns": True})
            record("second_process_read", {"id_matches": read["thread"]["id"] == proxy_id,
                                           "turn_count": len(read["thread"].get("turns", []))})
            try:
                await peer.call("thread/resume", {"threadId": proxy_id})
                record("second_process_resume_while_owner_alive", {"accepted": True})
            except RuntimeError as error:
                record("second_process_resume_while_owner_alive", {"accepted": False, "message": str(error)})
        finally:
            await peer.close()
        await rpc.close()
        rpc = await boot("proxy")
        await rpc.call("thread/resume", {"threadId": proxy_id})
        record("proxy_resume_after_restart", await rpc.turn(proxy_id, "What marker did I ask you to remember? Reply only that marker."))
        await rpc.close()
        rpc = await boot("normal")
        record("restored_identity", await identity(rpc))
        default_list = await rpc.call("thread/list", {"limit": 20})
        record("restored_default_list", {"contains_proxy": any(x["id"] == proxy_id for x in default_list["data"]),
                                         "contains_normal": any(x["id"] == normal_id for x in default_list["data"])})
        await rpc.call("thread/resume", {"threadId": normal_id})
        record("restored_normal_resume", await rpc.turn(normal_id, "What marker did I ask you to remember? Reply only that marker."))
        await rpc.call("thread/resume", {"threadId": proxy_id, "modelProvider": "openai"})
        record("restored_pool_resume", await rpc.turn(proxy_id, "What marker did I ask you to remember? Reply only that marker."))
    except Exception as error:
        # Exception texts are redacted before leaving the private directory.
        message = str(error).replace(state["key"], "[PROXY_KEY]").replace(state["token"], "[CHATGPT_TOKEN]")
        record("probe_error", {"type": type(error).__name__, "message": message})
    finally:
        if rpc:
            await rpc.close()
        if observer:
            observer.shutdown()
        if proxy and proxy.returncode is None:
            proxy.send_signal(signal.SIGINT)
            try:
                await asyncio.wait_for(proxy.wait(), 10)
            except asyncio.TimeoutError:
                proxy.kill()
                await proxy.wait()
        record("live_files_unchanged", hashes(Path.home() / ".codex") == state["source_hashes"])
        record("isolated_auth_unchanged", (state["home"] / "auth.json").read_bytes() ==
               (Path.home() / ".codex/auth.json").read_bytes())
        print("report", state["root"] / "report.json", flush=True)


if __name__ == "__main__":
    asyncio.run(main())
