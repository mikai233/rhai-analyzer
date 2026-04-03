import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientRoot = path.resolve(scriptDir, "..");
const artifactDir = path.join(clientRoot, ".artifacts");
const serverDir = path.join(clientRoot, "server");
const target = process.env.VSCODE_TARGET || process.env.npm_config_target || "";

assertServerBundlePrepared();

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

function assertServerBundlePrepared() {
    const targetsPath = path.join(serverDir, "targets.json");
    if (!fs.existsSync(targetsPath)) {
        console.error("No staged Rhai LSP bundle found in clients/vscode/server.");
        console.error("Run `npm run prepare-server` first, or use `npm run package:local`.");
        process.exit(1);
    }

    let manifest;
    try {
        manifest = JSON.parse(fs.readFileSync(targetsPath, "utf8"));
    } catch (error) {
        console.error(`Failed to read staged server manifest at ${targetsPath}.`);
        console.error(error instanceof Error ? error.message : String(error));
        process.exit(1);
    }

    const targets = Array.isArray(manifest?.targets) ? manifest.targets : [];
    if (targets.length === 0) {
        console.error(`Staged server manifest at ${targetsPath} does not list any targets.`);
        console.error("Run `npm run prepare-server` first, or use `npm run package:local`.");
        process.exit(1);
    }

    for (const target of targets) {
        const executable = target.startsWith("win32-") ? "rhai-lsp.exe" : "rhai-lsp";
        const binaryPath = path.join(serverDir, target, executable);
        if (!fs.existsSync(binaryPath)) {
            console.error(`Staged server target ${target} is missing its executable at ${binaryPath}.`);
            process.exit(1);
        }
    }
}
