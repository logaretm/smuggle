import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// Changesets versions exactly one package: npm/smuggle-cli. Every other place
// that carries a version number is derived from it, which is what this script
// does. It runs straight after `changeset version`, so the release PR arrives
// with the crate, the lockfile, and the platform packages already in step.

const scriptsDir: string = dirname(fileURLToPath(import.meta.url));
const rootDir: string = join(scriptsDir, "..");
const npmDir: string = join(rootDir, "npm");

const platformDirs: string[] = [
  "darwin-arm64",
  "darwin-x64",
  "linux-arm64-gnu",
  "linux-x64-gnu",
];

const rootPkgPath: string = join(npmDir, "smuggle-cli", "package.json");
const rootPkg = JSON.parse(readFileSync(rootPkgPath, "utf8"));
const version: string = rootPkg.version;

if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`smuggle-cli has no usable version: ${version}`);
  process.exit(1);
}

// The optional deps pin an exact version, so they move with the binaries they
// point at. A stale pin here resolves to no binary at all at install time.
for (const dep of Object.keys(rootPkg.optionalDependencies)) {
  rootPkg.optionalDependencies[dep] = version;
}
writeFileSync(rootPkgPath, JSON.stringify(rootPkg, null, 2) + "\n");

for (const dir of platformDirs) {
  const pkgPath: string = join(npmDir, dir, "package.json");
  const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
  pkg.version = version;
  writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
}

// Only the `[package]` table, which Cargo requires to come first, so the first
// `version =` in the file is the crate's own and not a dependency's.
const cargoTomlPath: string = join(rootDir, "Cargo.toml");
const cargoToml: string = readFileSync(cargoTomlPath, "utf8");
const bumpedToml: string = cargoToml.replace(
  /^version = ".*"$/m,
  `version = "${version}"`,
);
if (bumpedToml === cargoToml && !cargoToml.includes(`version = "${version}"`)) {
  console.error("Could not find the crate version in Cargo.toml");
  process.exit(1);
}
writeFileSync(cargoTomlPath, bumpedToml);

// Editing the lockfile by hand rather than shelling out to `cargo update` keeps
// the release PR job free of a Rust toolchain. Safe because a workspace member
// entry has no checksum and no source, just the version CI builds with
// `--locked`.
const cargoLockPath: string = join(rootDir, "Cargo.lock");
const cargoLock: string = readFileSync(cargoLockPath, "utf8");
const memberEntry = /(\[\[package\]\]\nname = "smuggle"\nversion = )".*"/;
if (!memberEntry.test(cargoLock)) {
  console.error("Could not find the smuggle package entry in Cargo.lock");
  process.exit(1);
}
writeFileSync(cargoLockPath, cargoLock.replace(memberEntry, `$1"${version}"`));

console.log(`Synced Cargo.toml, Cargo.lock, and npm/ to ${version}`);
