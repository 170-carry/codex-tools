import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const rootPackagePath = path.join(repoRoot, "package.json");
const npmRoot = path.join(repoRoot, "npm");
const rootPackage = JSON.parse(fs.readFileSync(rootPackagePath, "utf8"));
const packageDirs = fs
  .readdirSync(npmRoot, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort();
const nativePackages = packageDirs
  .filter((name) => name !== "ctc")
  .map((name) => `@170-carry/${name}`)
  .sort();

for (const dir of packageDirs) {
  const packagePath = path.join(npmRoot, dir, "package.json");
  const packageJson = JSON.parse(fs.readFileSync(packagePath, "utf8"));
  packageJson.version = rootPackage.version;

  if (packageJson.name === "@170-carry/ctc") {
    packageJson.optionalDependencies = Object.fromEntries(
      nativePackages.map((packageName) => [packageName, rootPackage.version]),
    );
  }

  fs.writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);
}
