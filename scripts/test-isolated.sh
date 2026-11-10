#!/usr/bin/env bash
# Run debug-build tests with synthetic app/Codex directories. Never override HOME.
set -euo pipefail
codex_tools_repo=$(cd "$(dirname "$0")/.." && pwd)
codex_tools_test_root=$(mktemp -d "${TMPDIR:-/tmp}/codex-tools-tests.XXXXXX")
trap 'rm -rf "$codex_tools_test_root"' EXIT
mkdir -p "$codex_tools_test_root/app" "$codex_tools_test_root/codex"
export CODEX_TOOLS_DEV_DATA_DIR="$codex_tools_test_root/app"
export CODEX_TOOLS_DEV_CODEX_DIR="$codex_tools_test_root/codex"
export CODEX_TOOLS_TEST_GUARD=1
cd "$codex_tools_repo"
if [[ "$(uname -s)" == Darwin ]]; then
  python3 - "$codex_tools_test_root/sandbox.sb" "$HOME" "$codex_tools_repo" <<'PY'
import json
import pathlib
import sys

output, user_home, repo = sys.argv[1:]
protected = [
    '.codex',
    'Library/Application Support/com.carry.codex-tools',
    'Library/Application Support/Codex',
    'Library/Application Support/com.openai.codex',
]
rules = ['(version 1)', '(allow default)', '(deny signal)',
         '(deny process-exec (literal "/usr/bin/open") (literal "/usr/bin/osascript") (subpath "/Applications"))']
for relative in protected:
    rules.append('(deny file-read* file-write* (subpath %s))' % json.dumps(str(pathlib.Path(user_home) / relative), ensure_ascii=False))
# Allow compiler caches and this checkout, but prohibit writes to other home files.
rules.append('(deny file-write* (require-all (subpath %s) (require-not (subpath %s)) (require-not (subpath %s))))' %
             (json.dumps(user_home, ensure_ascii=False), json.dumps(repo, ensure_ascii=False), json.dumps(str(pathlib.Path(user_home) / '.cargo'), ensure_ascii=False)))
pathlib.Path(output).write_text('\n'.join(rules) + '\n')
PY
  exec_status=0
  /usr/bin/sandbox-exec -f "$codex_tools_test_root/sandbox.sb" "$@" || exec_status=$?
  exit "$exec_status"
fi
echo 'This guarded test runner currently requires macOS sandbox-exec.' >&2
exit 1
