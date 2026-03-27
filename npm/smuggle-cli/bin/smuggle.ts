#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { join } from "node:path";

const PLATFORMS: Record<string, string> = {
  "darwin arm64": "@smuggle-cli/darwin-arm64",
  "darwin x64": "@smuggle-cli/darwin-x64",
  "linux arm64": "@smuggle-cli/linux-arm64-gnu",
  "linux x64": "@smuggle-cli/linux-x64-gnu",
};

const key: string = `${process.platform} ${process.arch}`;
const pkg: string | undefined = PLATFORMS[key];

if (!pkg) {
  console.error(
    `Unsupported platform: ${process.platform} ${process.arch}\n` +
      `smuggle-cli currently supports: ${Object.keys(PLATFORMS).join(", ")}`
  );
  process.exit(1);
}

const require_ = createRequire(import.meta.url);

let bin: string;
try {
  bin = require_.resolve(join(pkg, "bin/smuggle"));
} catch {
  console.error(
    `Could not find the binary for your platform (${key}).\n` +
      `Expected package: ${pkg}\n\n` +
      `Try reinstalling smuggle-cli:\n` +
      `  npm install -g smuggle-cli`
  );
  process.exit(1);
}

const result = spawnSync(bin, process.argv.slice(2), {
  stdio: "inherit",
});

process.exit(result.status ?? 1);
