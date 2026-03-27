import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { execSync } from "node:child_process";
import { join } from "node:path";
import { tmpdir } from "node:os";

const packages: string[] = [
  "smuggle-cli",
  "@smuggle-cli/darwin-arm64",
  "@smuggle-cli/darwin-x64",
  "@smuggle-cli/linux-arm64-gnu",
  "@smuggle-cli/linux-x64-gnu",
];

for (const name of packages) {
  const dir: string = mkdtempSync(join(tmpdir(), "smuggle-init-"));

  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify(
      {
        name,
        version: "0.0.0",
        description: "Placeholder for initial publish",
        publishConfig: { access: "public" },
      },
      null,
      2
    ) + "\n"
  );

  const otp: string | undefined = process.argv.find((a) => a.startsWith("--otp="));
  const otpFlag: string = otp ? ` ${otp}` : "";

  try {
    execSync(`npm view ${name}@0.0.0 version`, { stdio: "ignore" });
    console.log(`${name}@0.0.0 already published, skipping.`);
  } catch {
    console.log(`Publishing ${name}@0.0.0...`);
    execSync(`npm publish --access public${otpFlag}`, { cwd: dir, stdio: "inherit" });
  }
  rmSync(dir, { recursive: true });
}

console.log("\nAll packages published. Now configure trusted publishers on npmjs.com.");
