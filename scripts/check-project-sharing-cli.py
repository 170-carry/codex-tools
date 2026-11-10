"""Exercise the real CLI dry-run using wholly synthetic data, behind the OS guard."""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile


def fake_auth(name):
    claims = {"exp": 4102444800, "https://api.openai.com/auth": {
        "chatgpt_user_id": f"synthetic-user-{name}",
        "chatgpt_account_id": f"synthetic-account-{name}",
    }}
    payload = base64.urlsafe_b64encode(json.dumps(claims).encode()).decode().rstrip("=")
    token = f"synthetic.{payload}.not-a-signature"
    return {"tokens": {"id_token": token, "access_token": token,
                       "refresh_token": "synthetic-not-a-credential",
                       "account_id": f"synthetic-account-{name}"}}


def fingerprint_tree(root):
    return {str(path.relative_to(root)): "directory" if path.is_dir() else
            hashlib.sha256(path.read_bytes()).hexdigest() for path in root.rglob("*")}


def fixture(root):
    home, data = root / "codex", root / "data"
    home.mkdir()
    data.mkdir()
    accounts = [{"id": name, "label": f"Fictional {name}",
                 "accountId": f"synthetic-account-{name}", "authJson": fake_auth(name),
                 "addedAt": 1, "updatedAt": 1} for name in ("a", "b", "c")]
    (data / "accounts.json").write_text(json.dumps({"version": 2, "accounts": accounts,
        "settings": {"activeAccountId": "a", "preserveLocalProjects": True}}))
    (home / "auth.json").write_text(json.dumps(fake_auth("a")))
    (home / "config.toml").write_text('model_provider = "openai"\n')
    (home / "synthetic-session.jsonl").write_text('{"synthetic":true}\n')
    owner = hashlib.sha256(json.dumps(["synthetic-user-a", "synthetic-account-a"],
                                     separators=(",", ":")).encode()).hexdigest()
    projects, mappings, memberships, unassigned = {}, {}, {}, []
    with sqlite3.connect(home / "state_5.sqlite") as db:
        db.executescript("CREATE TABLE projects(id TEXT PRIMARY KEY,name TEXT,metadata TEXT NOT NULL DEFAULT '{}');"
            "CREATE TABLE project_roots(project_id TEXT,position INTEGER,path TEXT);"
            "CREATE TABLE threads(id TEXT PRIMARY KEY,project_id TEXT);")
        for index in range(24):
            key, server = f"synthetic-project-{index}", f"synthetic-server-{index}"
            name, path = f"Fictional project {index}", f"/synthetic-only/workspace-{index}"
            projects[key] = {"id": key, "name": name, "rootPaths": [path],
                             "createdAt": 1, "updatedAt": 1}
            mappings[key] = server
            db.execute("INSERT INTO projects(id,name) VALUES(?,?)", (server, name))
            db.execute("INSERT INTO project_roots VALUES(?,0,?)", (server, path))
        for index in range(81):
            key = f"synthetic-thread-{index}"
            memberships[key] = {"projectKind": "local", "projectId": f"synthetic-project-{index % 24}"}
            db.execute("INSERT INTO threads(id) VALUES(?)", (key,))
        for index in range(4):
            key = f"synthetic-unassigned-{index}"
            unassigned.append(key)
            db.execute("INSERT INTO threads(id) VALUES(?)", (key,))
    db.close()
    state = {"electron-account-renderer-state-v1": {"owner": owner, "inactive": {}},
             "local-projects": projects, "project-order": list(projects),
             "thread-project-assignments": memberships, "projectless-thread-ids": unassigned,
             "app-server-project-id-by-legacy-project-id-by-host": {f"local:{home}": mappings},
             "unrelated": {"permission": "keep"}}
    for name in (".codex-global-state.json", ".codex-global-state.json.bak"):
        (home / name).write_text(json.dumps(state))
    return home, data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    args = parser.parse_args()
    if os.environ.get("CODEX_TOOLS_TEST_GUARD") != "1":
        parser.error("Run through scripts/test-isolated.sh; do not use a real account directory.")
    with tempfile.TemporaryDirectory(prefix="codex-tools-cli-smoke-") as directory:
        root = Path(directory).resolve()
        home, data = fixture(root)
        command = [str(args.cli.resolve()), "--data-dir", str(data), "--codex-home", str(home),
                   "--json", "switch", "b", "--preserve-local-projects", "--dry-run"]
        before = fingerprint_tree(root)
        output = subprocess.run(command, text=True, capture_output=True, check=True, timeout=30)
        preview = json.loads(output.stdout)
        assert (preview["projectCount"], preview["threadLinkCount"], preview["unassignedThreadCount"]) == (24, 81, 4)
        assert not preview["requiresMergeConfirmation"] and not preview["noOp"]
        assert fingerprint_tree(root) == before, "dry-run mutated a source file or directory"
        state = json.loads((home / ".codex-global-state.json").read_text())
        state["thread-project-assignments"]["synthetic-unresolved"] = {
            "projectKind": "local", "projectId": "synthetic-missing-project"}
        for name in (".codex-global-state.json", ".codex-global-state.json.bak"):
            (home / name).write_text(json.dumps(state))
        with sqlite3.connect(home / "state_5.sqlite") as db:
            db.execute("DELETE FROM threads WHERE id IN ('synthetic-thread-0','synthetic-unassigned-0')")
        db.close()
        before = fingerprint_tree(root)
        output = subprocess.run(command, text=True, capture_output=True, check=True, timeout=30)
        preview = json.loads(output.stdout)
        assert (preview["projectCount"], preview["threadLinkCount"], preview["unassignedThreadCount"]) == (24, 81, 4)
        assert fingerprint_tree(root) == before, "compatibility dry-run mutated source data"
        (home / "config.toml").write_text('model_provider = "openai"\nchatgpt_base_url = "https://synthetic.invalid"\n')
        before = fingerprint_tree(root)
        rejected = subprocess.run(command, text=True, capture_output=True, timeout=30)
        assert rejected.returncode != 0, "unsupported route was accepted"
        assert fingerprint_tree(root) == before, "rejected dry-run mutated source data"
        (home / "config.toml").write_text('model_provider = "openai"\n')
        with sqlite3.connect(home / "state_5.sqlite") as db:
            binding = json.dumps({"codex.chatGptBacking": json.dumps({
                "userId": "synthetic-user-a", "accountId": "synthetic-account-a",
                "projectId": "synthetic-cloud-project",
            })})
            db.execute("UPDATE projects SET metadata=? WHERE id=?", (binding, "synthetic-server-0"))
        db.close()
        before = fingerprint_tree(root)
        rejected = subprocess.run(command, text=True, capture_output=True, timeout=30)
        assert rejected.returncode != 0, "a database cloud binding was accepted"
        assert fingerprint_tree(root) == before, "cloud rejection mutated source data"
        print("CLI smoke passed: 24 synthetic projects, 81 links, 4 unassigned; unresolved project reference and absent thread index rows supported; dry-run, route rejection, and cloud-binding rejection left all source files/directories unchanged.")


if __name__ == "__main__":
    main()
