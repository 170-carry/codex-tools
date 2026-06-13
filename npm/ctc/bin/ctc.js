#!/usr/bin/env node
"use strict";

const { existsSync } = require("node:fs");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");

const PACKAGE_NAME = "@170-carry/ctc";
const RELEASE_URL = "https://github.com/170-carry/codex-tools/releases/latest";
const NATIVE_PACKAGES = {
  "darwin:arm64": {
    name: "@170-carry/ctc-darwin-arm64",
    bin: "codex-tools-cli",
  },
  "darwin:x64": {
    name: "@170-carry/ctc-darwin-x64",
    bin: "codex-tools-cli",
  },
  "win32:x64": {
    name: "@170-carry/ctc-win32-x64",
    bin: "codex-tools-cli.exe",
  },
};

function main() {
  const args = process.argv.slice(2);
  if (args.length === 0 || isHelpCommand(args)) {
    printHelp();
    process.exit(0);
  }

  if (args[0] === "ui") {
    openDesktopUi(args.slice(1));
    return;
  }

  const nativeBin = resolveNativeBinary();
  runChild(nativeBin, args);
}

function isHelpCommand(args) {
  if (args[0] === "-h" || args[0] === "--help") {
    return true;
  }
  return args[0] === "help" && (args.length === 1 || args[1] === "ui");
}

function printHelp() {
  const version = require("../package.json").version;
  console.log(`ctc ${version}

Usage:
  ctc ui
  ctc list [--json]
  ctc switch <account> [--json]
  ctc switch --best [--launch]
  ctc login [--label <name>]
  ctc import <path...> [--json]
  ctc export [path] [--json]
  ctc usage [account] [--cached] [--json]
  ctc doctor [--json]
  ctc report [--json]
  ctc tui

Commands:
  ui       Open the installed Codex Tools desktop app
  list     List stored accounts
  switch   Switch ~/.codex/auth.json to one stored account
  login    Run official codex login, then import the auth result
  import   Import auth/account JSON files or current ~/.codex/auth.json
  export   Export stored accounts as JSON
  usage    Refresh or show account usage
  doctor   Check local paths, Codex CLI, auth files, and account store
  report   Print a full diagnostic report
  tui      Open a terminal account selector

Run "ctc <command> --help" for command-specific options.`);
}

function resolveNativeBinary() {
  if (process.env.CTC_NATIVE_BIN) {
    return process.env.CTC_NATIVE_BIN;
  }

  const nativePackage = nativePackageForCurrentPlatform();
  let packageJsonPath;
  try {
    packageJsonPath = require.resolve(`${nativePackage.name}/package.json`);
  } catch {
    const platformKey = platformKeyForCurrentProcess();
    console.error(`Missing native package for ${platformKey}: ${nativePackage.name}`);
    console.error(`Reinstall with optional dependencies enabled: npm i -g ${PACKAGE_NAME} --include=optional`);
    process.exit(1);
  }

  const binaryPath = path.join(path.dirname(packageJsonPath), "bin", nativePackage.bin);
  if (!existsSync(binaryPath)) {
    console.error(`Native binary is missing: ${binaryPath}`);
    console.error(`Reinstall ${PACKAGE_NAME} or report a broken package release.`);
    process.exit(1);
  }
  return binaryPath;
}

function nativePackageForCurrentPlatform() {
  const platformKey = platformKeyForCurrentProcess();
  const nativePackage = NATIVE_PACKAGES[platformKey];
  if (!nativePackage) {
    console.error(`Unsupported platform: ${platformKey}`);
    console.error(`Supported platforms: ${Object.keys(NATIVE_PACKAGES).join(", ")}`);
    process.exit(1);
  }
  return nativePackage;
}

function platformKeyForCurrentProcess() {
  return `${process.platform}:${process.arch}`;
}

function runChild(command, args) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    windowsHide: false,
  });

  if (result.error) {
    console.error(`Failed to run ${command}: ${result.error.message}`);
    process.exit(1);
  }

  process.exit(typeof result.status === "number" ? result.status : 1);
}

function openDesktopUi(args) {
  if (args.includes("-h") || args.includes("--help")) {
    console.log(`Usage:
  ctc ui

Open the installed Codex Tools desktop app.

If the desktop app is not installed, download it from:
  ${RELEASE_URL}`);
    process.exit(0);
  }

  if (process.env.CTC_DESKTOP_APP) {
    runDetached(process.env.CTC_DESKTOP_APP, []);
    return;
  }

  if (process.platform === "darwin") {
    const result = spawnSync("open", ["-a", "Codex Tools"], {
      stdio: "inherit",
      windowsHide: false,
    });
    if (!result.error && result.status === 0) {
      process.exit(0);
    }
    printDesktopUiInstallHint();
    process.exit(1);
  }

  if (process.platform === "win32") {
    const candidate = findWindowsDesktopApp();
    if (candidate) {
      runDetached(candidate, []);
      return;
    }

    const result = spawnSync(
      "powershell.exe",
      ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", "Start-Process 'Codex Tools'"],
      {
        stdio: "ignore",
        windowsHide: false,
      },
    );
    if (!result.error && result.status === 0) {
      process.exit(0);
    }
    printDesktopUiInstallHint();
    process.exit(1);
  }

  printDesktopUiInstallHint();
  process.exit(1);
}

function findWindowsDesktopApp() {
  const candidates = [
    process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, "Codex Tools", "Codex Tools.exe"),
    process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, "Programs", "Codex Tools", "Codex Tools.exe"),
    process.env.PROGRAMFILES && path.join(process.env.PROGRAMFILES, "Codex Tools", "Codex Tools.exe"),
    process.env["PROGRAMFILES(X86)"] && path.join(process.env["PROGRAMFILES(X86)"], "Codex Tools", "Codex Tools.exe"),
  ].filter(Boolean);

  return candidates.find((candidate) => existsSync(candidate)) || null;
}

function runDetached(command, args) {
  if (path.isAbsolute(command) && !existsSync(command)) {
    console.error(`Failed to open Codex Tools: ${command} does not exist`);
    process.exit(1);
  }

  if (process.platform === "win32") {
    const result = spawnSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        `Start-Process -FilePath ${quotePowerShellString(command)}`,
      ],
      {
        stdio: "ignore",
        windowsHide: false,
      },
    );
    if (result.error) {
      console.error(`Failed to open Codex Tools: ${result.error.message}`);
      process.exit(1);
    }
    process.exit(typeof result.status === "number" ? result.status : 1);
  }

  const child = spawn(command, args, {
    detached: true,
    stdio: "ignore",
    windowsHide: false,
  });

  child.unref();
  process.exit(0);
}

function quotePowerShellString(value) {
  return `'${value.replace(/'/g, "''")}'`;
}

function printDesktopUiInstallHint() {
  console.error("Codex Tools desktop app is not installed or cannot be found.");
  console.error(`Install it from: ${RELEASE_URL}`);
}

main();
