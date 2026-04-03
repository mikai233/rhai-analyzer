import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientRoot = path.resolve(scriptDir, "..");
const artifactDir = path.join(clientRoot, ".artifacts");
const target = process.env.VSCODE_TARGET || process.env.npm_config_target || "";

fs.mkdirSync(artifactDir, { recursive: true });

const fileName = target
    ? `rhai-analyzer-${target}.vsix`
    : "rhai-analyzer.vsix";
const outputPath = path.join(artifactDir, fileName);

const args = ["exec", "--", "vsce", "package", "--out", outputPath];
if (target) {
    args.push("--target", target);
}

const result = spawnSync("npm", args, {
    cwd: clientRoot,
    env: {
        ...process.env,
        RHAI_SKIP_VSCODE_PREPUBLISH: "1",
    },
    stdio: "inherit",
    shell: process.platform === "win32",
});

if (result.status !== 0) {
    process.exit(result.status ?? 1);
}
