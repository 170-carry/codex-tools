"""Prepare private disposable data. Never mutate the live Codex configuration."""
import asyncio
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import secrets
import time

CODEX = "/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex"
PROXY = "/Applications/Codex Tools.app/Contents/MacOS/codex-tools-proxyd"


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def private_write(path, content):
    path.write_text(content)
    path.chmod(0o600)


def prepare(root):
    source = Path.home() / ".codex"
    data = Path.home() / "Library/Application Support/com.carry.codex-tools"
    home = root / "codex-home"
    pool = root / "pool"
    for path in [home, pool, root / "workspace"]:
        path.mkdir(exist_ok=True, mode=0o700)
    for name in ["auth.json", "models_catalog_ultrafast.json"]:
        shutil.copy2(source / name, home / name)
        (home / name).chmod(0o600)
    auth = json.loads((home / "auth.json").read_text())
    store = json.loads((data / "accounts.json").read_text())
    active = auth["tokens"]["account_id"]
    def fresh_access_token(account):
        token = account.get("authJson", {}).get("tokens", {}).get("access_token", "")
        try:
            payload = json.loads(base64.urlsafe_b64decode(token.split(".")[1] + "==="))
            return payload["exp"] - time.time() > 1800
        except (KeyError, ValueError, IndexError):
            return False

    candidates = [a for a in store["accounts"] if
                  a.get("authJson", {}).get("tokens", {}).get("account_id") != active
                  and not a.get("authRefreshBlocked") and a.get("apiProxyEnabled")
                  and fresh_access_token(a)]
    if not candidates:
        raise RuntimeError("Need a separate pool account with an already-valid access token")
    selected = candidates[-1]
    selected["profileAuthPath"] = None
    selected["profileConfigPath"] = None
    # Keep every credential in private temporary storage. One fixed pool account
    # makes identity separation and later restoration failures attributable.
    store["accounts"] = [selected]
    store["settings"]["autoAccountWarmupEnabled"] = False
    store["settings"]["apiProxyLoadBalanceMode"] = "average"
    private_write(pool / "accounts.json", json.dumps(store))
    key = "sk-ctc229-" + secrets.token_hex(24)
    stamp = int(time.time())
    private_write(pool / "api-proxy.key", key)
    private_write(pool / "api-proxy-keys.json", json.dumps({
        "version": 1, "updatedAt": stamp, "keys": [{
            "id": "ctc229-probe", "label": "Isolated compatibility probe",
            "key": key, "enabled": True, "allowedModels": ["gpt-6.1-sol"],
            "allowedReasoningEfforts": ["low"], "allowedServiceTiers": ["default"],
            "createdAt": stamp, "updatedAt": stamp,
        }],
    }))
    store["settings"]["apiProxyApiKey"] = key
    private_write(pool / "accounts.json", json.dumps(store))
    env = dict(os.environ, CODEX_HOME=str(home), CODEX_TOOLS_PROXY_SERVICE_TIER="default")
    bypass = env.get("NO_PROXY", env.get("no_proxy", ""))
    env["NO_PROXY"] = env["no_proxy"] = bypass + ",127.0.0.1,localhost,::1"
    for name in ["OPENAI_API_KEY", "OPENAI_BASE_URL"]:
        env.pop(name, None)
    return {"root": root, "home": home, "pool": pool, "env": env,
            "key": key, "token": auth["tokens"]["access_token"],
            "proxy_port": free_port(), "observer_port": free_port(),
            "source_hashes": hashes(source),
            "upstream_differs_from_login": selected["authJson"]["tokens"]["account_id"] != active}


def hashes(home):
    return {name: hashlib.sha256((home / name).read_bytes()).hexdigest()
            for name in ["auth.json", "config.toml"]}


def configure(state, proxy):
    home = state["home"]
    base = ('model = "gpt-6.1-sol"\ncli_auth_credentials_store = "file"\n'
            'model_reasoning_effort = "low"\nservice_tier = "default"\n'
            f'model_catalog_json = {json.dumps(str(home / "models_catalog_ultrafast.json"))}\n')
    if proxy:
        base += ('model_provider = "ctc_remote_probe"\n'
                 '[model_providers.ctc_remote_probe]\nname = "CTC Remote Probe"\n'
                 f'base_url = "http://127.0.0.1:{state["observer_port"]}/v1"\n'
                 'wire_api = "responses"\nrequires_openai_auth = true\n'
                 'supports_websockets = false\nrequest_max_retries = 0\n'
                 'stream_max_retries = 0\n'
                 '[model_providers.ctc_remote_probe.http_headers]\n'
                 f'x-api-key = {json.dumps(state["key"])}\n')
    else:
        base += 'model_provider = "openai"\n'
    private_write(home / "config.toml", base)


async def start_proxy(state):
    log = open(state["root"] / "proxy-private.log", "wb")
    process = await asyncio.create_subprocess_exec(
        PROXY, "serve", "--host", "127.0.0.1", "--port", str(state["proxy_port"]),
        "--data-dir", str(state["pool"]), "--no-sync-current-auth",
        env=state["env"], stdout=log, stderr=log,
    )
    log.close()
    for _ in range(100):
        if process.returncode is not None:
            raise RuntimeError("Isolated proxy exited; inspect private log")
        try:
            reader, writer = await asyncio.open_connection("127.0.0.1", state["proxy_port"])
            writer.close()
            await writer.wait_closed()
            return process
        except OSError:
            await asyncio.sleep(0.1)
    process.terminate()
    raise TimeoutError("Isolated proxy failed to listen")
