import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const version: string | undefined = process.argv[2];
if (!version) {
  console.error("Usage: node update-versions.ts <version>");
  process.exit(1);
}

const scriptsDir: string = dirname(fileURLToPath(import.meta.url));
const npmDir: string = join(scriptsDir, "..", "npm");

const platformDirs: string[] = [
  "darwin-arm64",
  "darwin-x64",
  "linux-arm64-gnu",
  "linux-x64-gnu",
];

for (const dir of platformDirs) {
  const pkgPath: string = join(npmDir, dir, "package.json");
  const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
  pkg.version = version;
  writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
}

const rootPkgPath: string = join(npmDir, "smuggle-cli", "package.json");
const rootPkg = JSON.parse(readFileSync(rootPkgPath, "utf8"));
rootPkg.version = version;
for (const dep of Object.keys(rootPkg.optionalDependencies)) {
  rootPkg.optionalDependencies[dep] = version;
}
writeFileSync(rootPkgPath, JSON.stringify(rootPkg, null, 2) + "\n");

console.log(`Updated all npm packages to ${version}`);
