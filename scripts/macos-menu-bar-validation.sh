#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/macos-menu-bar-validation.sh <dev|acceptance> [--no-launch]

Profiles:
  dev         Stable development Bundle ID with isolated runtime data.
  acceptance  Production Bundle ID for final macOS menu-bar validation,
              while still using isolated runtime data.

Debug builds add "Open Quota Setup Preview" to the application menu and a
matching action in Settings. It reopens the real first-launch quota setup
without changing Bundle IDs or clearing application data.

Optional environment overrides:
  CODEX_TOOLS_MACOS_VALIDATION_RUNTIME_ROOT
  CODEX_TOOLS_MACOS_VALIDATION_CODEX_DIR
  CODEX_TOOLS_MACOS_VALIDATION_DATA_DIR
  CODEX_TOOLS_MACOS_VALIDATION_CONTROL_CENTER_PLIST

The default isolated runtime is stored under:
  ~/Library/Application Support/Codex Tools Menu Bar Validation
EOF
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This validation script only runs on macOS." >&2
  exit 1
fi

profile="${1:-}"
launch_app=true
if [[ "${2:-}" == "--no-launch" ]]; then
  launch_app=false
elif [[ -n "${2:-}" ]]; then
  usage >&2
  exit 2
fi

case "$profile" in
  dev)
    product_name="Codex Tools Menu Bar Dev"
    bundle_id="com.carry.codex-tools.menubar-dev"
    ;;
  acceptance)
    product_name="Codex Tools Menu Bar Acceptance"
    bundle_id="com.carry.codex-tools"
    ;;
  -h|--help|"")
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
runtime_root="${CODEX_TOOLS_MACOS_VALIDATION_RUNTIME_ROOT:-$HOME/Library/Application Support/Codex Tools Menu Bar Validation}"
profile_runtime="$runtime_root/$profile"
codex_dir="${CODEX_TOOLS_MACOS_VALIDATION_CODEX_DIR:-$profile_runtime/codex}"
data_dir="${CODEX_TOOLS_MACOS_VALIDATION_DATA_DIR:-$profile_runtime/data}"
bundle_path="$repo_root/src-tauri/target/debug/bundle/macos/$product_name.app"
info_plist="$bundle_path/Contents/Info.plist"

mkdir -p "$runtime_root" "$profile_runtime" "$codex_dir" "$data_dir"
chmod 700 "$runtime_root" "$profile_runtime"
if [[ "$codex_dir" == "$profile_runtime/"* ]]; then
  chmod 700 "$codex_dir"
fi
if [[ "$data_dir" == "$profile_runtime/"* ]]; then
  chmod 700 "$data_dir"
fi

if [[ "$profile" == "acceptance" && "$bundle_id" != "com.carry.codex-tools" ]]; then
  echo "Acceptance builds must use the production Bundle ID." >&2
  exit 1
fi
if [[ "$profile" == "dev" && "$bundle_id" == "com.carry.codex-tools" ]]; then
  echo "Development builds must not use the production Bundle ID." >&2
  exit 1
fi

running_codex_tools_apps() {
  osascript -l JavaScript -e '
    ObjC.import("AppKit");
    var apps = $.NSWorkspace.sharedWorkspace.runningApplications;
    for (var i = 0; i < apps.count; i++) {
      var app = apps.objectAtIndex(i);
      var identifier = ObjC.unwrap(app.bundleIdentifier);
      if (identifier && identifier.indexOf("com.carry.codex-tools") === 0) {
        var url = app.bundleURL;
        var path = url ? ObjC.unwrap(url.path) : "<unknown path>";
        console.log(identifier + "\t" + path);
      }
    }
  ' 2>&1
}

reject_other_codex_tools_apps() {
  local running_apps other_apps
  running_apps="$(running_codex_tools_apps)"
  other_apps="$(awk -F '\t' -v target="$bundle_id" '$1 != target' <<<"$running_apps")"
  if [[ -z "$other_apps" ]]; then
    return
  fi

  echo "Another Codex Tools validation identity is already running:" >&2
  echo "$other_apps" >&2
  echo "Quit it before launching $bundle_id; parallel menu-bar identities are not accepted." >&2
  exit 1
}

check_control_center_records() {
  local control_center_plist tracked_json blocked_record foreign_references stale_identities
  control_center_plist="${CODEX_TOOLS_MACOS_VALIDATION_CONTROL_CENTER_PLIST:-$HOME/Library/Group Containers/group.com.apple.controlcenter/Library/Preferences/group.com.apple.controlcenter.plist}"

  if ! command -v jq >/dev/null 2>&1; then
    echo "Warning: jq is unavailable; the read-only Control Center pollution check was skipped." >&2
    return
  fi

  if ! tracked_json="$(
    plutil -extract trackedApplications raw -o - "$control_center_plist" 2>/dev/null \
      | base64 -D 2>/dev/null \
      | plutil -convert json -o - - 2>/dev/null
  )"; then
    echo "Warning: Control Center status-item records could not be read; the read-only pollution check was skipped." >&2
    echo "This is expected when the launching terminal does not have permission to read the Control Center group container." >&2
    return
  fi

  blocked_record="$(
    jq -r --arg target "$bundle_id" '
      .[]
      | select(type == "object" and has("isAllowed"))
      | select(.location.bundle._0 == $target and .isAllowed != true)
      | "\(.location.bundle._0) isAllowed=\(.isAllowed)"
    ' <<<"$tracked_json"
  )"
  foreign_references="$(
    jq -r --arg prefix "com.carry.codex-tools" '
      .[]
      | select(type == "object" and has("isAllowed"))
      | . as $entry
      | [
          (.menuItemLocations // [])[]?
          | .bundle?._0?
          | select(type == "string" and startswith($prefix))
        ] as $matches
      | select(($matches | length) > 0)
      | (.location.bundle._0 // .location.adhocBinary._0.relative // "<unknown owner>") as $owner
      | select(($owner | startswith($prefix)) | not)
      | "owner=\($owner) items=\($matches | join(",")) allowed=\($entry.isAllowed)"
    ' <<<"$tracked_json"
  )"
  stale_identities="$(
    jq -r \
      --arg prefix "com.carry.codex-tools" \
      --arg production "com.carry.codex-tools" \
      --arg development "com.carry.codex-tools.menubar-dev" '
        [
          .[]
          | select(type == "object" and has("isAllowed"))
          | .location.bundle._0?
          | select(
              type == "string"
              and startswith($prefix)
              and . != $production
              and . != $development
            )
        ]
        | unique[]
      ' <<<"$tracked_json"
  )"

  if [[ -n "$stale_identities" ]]; then
    echo "Warning: stale Codex Tools test identities remain in Control Center records:" >&2
    echo "$stale_identities" >&2
  fi
  if [[ -n "$blocked_record" || -n "$foreign_references" ]]; then
    echo "Control Center status-item pollution was detected; validation has been stopped." >&2
    if [[ -n "$blocked_record" ]]; then
      echo "$blocked_record" >&2
    fi
    if [[ -n "$foreign_references" ]]; then
      echo "$foreign_references" >&2
    fi
    echo "The script is read-only and did not modify Control Center data." >&2
    exit 1
  fi
}

reject_other_codex_tools_apps
check_control_center_records

build_config="{\"productName\":\"$product_name\",\"identifier\":\"$bundle_id\",\"bundle\":{\"targets\":[\"app\"],\"createUpdaterArtifacts\":false}}"

echo "Building $product_name ($bundle_id)..."
(
  cd "$repo_root"
  npm run tauri -- build --debug --config "$build_config"
)

if [[ ! -f "$info_plist" ]]; then
  echo "Expected application bundle was not generated: $bundle_path" >&2
  exit 1
fi

/usr/libexec/PlistBuddy -c "Delete :LSEnvironment" "$info_plist" >/dev/null 2>&1 || true
/usr/libexec/PlistBuddy -c "Add :LSEnvironment dict" "$info_plist"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:CODEX_TOOLS_DEV_CODEX_DIR string $codex_dir" "$info_plist"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:CODEX_TOOLS_DEV_DATA_DIR string $data_dir" "$info_plist"
codesign --force --deep --sign - "$bundle_path" >/dev/null

actual_bundle_id="$(/usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" "$info_plist")"
actual_codex_dir="$(/usr/libexec/PlistBuddy -c "Print :LSEnvironment:CODEX_TOOLS_DEV_CODEX_DIR" "$info_plist")"
actual_data_dir="$(/usr/libexec/PlistBuddy -c "Print :LSEnvironment:CODEX_TOOLS_DEV_DATA_DIR" "$info_plist")"

if [[ "$actual_bundle_id" != "$bundle_id" ]]; then
  echo "Bundle ID verification failed: expected $bundle_id, got $actual_bundle_id" >&2
  exit 1
fi
if [[ "$actual_codex_dir" != "$codex_dir" || "$actual_data_dir" != "$data_dir" ]]; then
  echo "Isolated runtime path verification failed." >&2
  exit 1
fi
codesign --verify --deep --strict "$bundle_path"

running_count() {
  osascript -e "tell application \"System Events\" to count (every application process whose bundle identifier is \"$bundle_id\")"
}

quit_matching_apps() {
  local count
  count="$(running_count)"
  if [[ "$count" == "0" ]]; then
    return
  fi

  echo "Stopping $count running app instance(s) with Bundle ID $bundle_id..."
  osascript -l JavaScript -e \
    "ObjC.import('AppKit'); var apps = $.NSRunningApplication.runningApplicationsWithBundleIdentifier('$bundle_id'); for (var i = 0; i < apps.count; i++) { apps.objectAtIndex(i).terminate; }" \
    >/dev/null

  for _ in 1 2 3 4 5; do
    if [[ "$(running_count)" == "0" ]]; then
      return
    fi
    sleep 1
  done

  echo "Graceful termination timed out; force-stopping only Bundle ID $bundle_id..."
  osascript -l JavaScript -e \
    "ObjC.import('AppKit'); var apps = $.NSRunningApplication.runningApplicationsWithBundleIdentifier('$bundle_id'); for (var i = 0; i < apps.count; i++) { apps.objectAtIndex(i).forceTerminate; }" \
    >/dev/null
  sleep 1
  if [[ "$(running_count)" != "0" ]]; then
    echo "A conflicting app with Bundle ID $bundle_id is still running." >&2
    exit 1
  fi
}

if [[ "$launch_app" == true ]]; then
  quit_matching_apps
  open -na "$bundle_path"

  executable_path="$bundle_path/Contents/MacOS/app"
  for _ in 1 2 3 4 5; do
    if pgrep -f "$executable_path" >/dev/null; then
      echo "Launched: $bundle_path"
      echo "Bundle ID: $actual_bundle_id"
      echo "Codex runtime: $actual_codex_dir"
      echo "Application data: $actual_data_dir"
      exit 0
    fi
    sleep 1
  done

  echo "The exact validation bundle did not start: $bundle_path" >&2
  exit 1
fi

echo "Built without launching: $bundle_path"
echo "Bundle ID: $actual_bundle_id"
echo "Codex runtime: $actual_codex_dir"
echo "Application data: $actual_data_dir"
