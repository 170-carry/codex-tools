import { build } from "esbuild";
import { mkdir, rm } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import path from "node:path";

// Server-side UI regression only: no browser, network, or real backend commands.
const output = path.resolve("node_modules/.tmp/pages-render-check.mjs");
await mkdir(path.dirname(output), { recursive: true });
try {
  await build({
    entryPoints: ["tests/ui/render-pages.tsx"],
    outfile: output,
    bundle: true,
    platform: "node",
    format: "esm",
    packages: "external",
    jsx: "automatic",
  });
  const result = spawnSync(process.execPath, [output], { stdio: "inherit" });
  process.exitCode = result.status ?? 1;
} finally {
  await rm(output, { force: true });
}
