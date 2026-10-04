import { build } from "esbuild";
import { mkdir, rm } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import path from "node:path";

// Server-side UI regression only: no browser, network, or real backend commands.
for (const entry of ["render-pages", "render-layouts"]) {
  const output = path.resolve(`node_modules/.tmp/${entry}-check.mjs`);
  await mkdir(path.dirname(output), { recursive: true });
  try {
    await build({
      entryPoints: [`tests/ui/${entry}.tsx`],
      outfile: output,
      bundle: true,
      platform: "node",
      format: "esm",
      packages: "external",
      jsx: "automatic",
    });
    const result = spawnSync(process.execPath, [output], { stdio: "inherit" });
    if (result.status !== 0) {
      process.exitCode = result.status ?? 1;
      break;
    }
  } finally {
    await rm(output, { force: true });
  }
}
