import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const [packageDir, sourceBinary, outputName] = process.argv.slice(2);

if (!packageDir || !sourceBinary || !outputName) {
  console.error("usage: node scripts/prepare-npm-native-package.mjs <package-dir> <source-binary> <output-name>");
  process.exit(1);
}

const sourcePath = path.resolve(repoRoot, sourceBinary);
const outputDir = path.resolve(repoRoot, packageDir, "bin");
const outputPath = path.join(outputDir, outputName);

if (!fs.existsSync(sourcePath)) {
  console.error(`source binary does not exist: ${sourcePath}`);
  process.exit(1);
}

fs.mkdirSync(outputDir, { recursive: true });
fs.copyFileSync(sourcePath, outputPath);

if (process.platform !== "win32") {
  fs.chmodSync(outputPath, 0o755);
}

console.log(`prepared ${outputPath}`);
