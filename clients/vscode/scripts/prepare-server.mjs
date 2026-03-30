import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { familySync, MUSL } = require("detect-libc");

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(clientRoot, "..", "..");
const serverDir = path.join(clientRoot, "server");

fs.rmSync(serverDir, { recursive: true, force: true });
fs.mkdirSync(serverDir, { recursive: true });

const bundledTargets = stageBundledServers();
if (bundledTargets.length === 0) {
    console.error("Could not locate any built rhai-lsp executables to stage.");
    process.exit(1);
}

fs.writeFileSync(
    path.join(serverDir, "targets.json"),
    JSON.stringify({ targets: bundledTargets }, null, 2),
);

console.log(`Staged Rhai LSP server binaries for: ${bundledTargets.join(", ")}`);

function stageBundledServers() {
    const stagedTargets = [];
    const serverManifest = process.env.RHAI_SERVER_MANIFEST?.trim();
    if (serverManifest) {
        const entries = JSON.parse(fs.readFileSync(serverManifest, "utf8"));
        for (const [target, source] of normalizeManifestEntries(entries)) {
            if (!fs.existsSync(source)) {
                throw new Error(`Bundled server target ${target} points to a missing file: ${source}`);
            }
            stageServerBinary(target, source);
            stagedTargets.push(target);
        }
        return stagedTargets;
    }

    const bundledServerDir = process.env.RHAI_SERVER_DIR?.trim();
    if (bundledServerDir) {
        for (const entry of fs.readdirSync(bundledServerDir, { withFileTypes: true })) {
            if (!entry.isDirectory()) {
                continue;
            }

            const target = entry.name;
            const source = path.join(
                bundledServerDir,
                target,
                executableNameForTarget(target),
            );
            if (!fs.existsSync(source)) {
                throw new Error(
                    `Bundled server target ${target} is missing its executable at ${source}.`,
                );
            }
            stageServerBinary(target, source);
            stagedTargets.push(target);
        }

        stagedTargets.sort();
        return stagedTargets;
    }

    const target = currentVsCodeTarget();
    if (!target) {
        console.error(
            `Unsupported host platform for local server staging: ${process.platform}-${process.arch}`,
        );
        process.exit(1);
    }

    const executable = executableNameForTarget(target);
    const explicitServer = process.env.RHAI_SERVER_PATH?.trim();
    const candidates = [
        explicitServer,
        path.join(repoRoot, "target", "release", executable),
        path.join(repoRoot, "target", "debug", executable),
    ].filter(Boolean);

    const serverPath = candidates.find((candidate) => fs.existsSync(candidate));
    if (!serverPath) {
        console.error("Could not locate a built rhai-lsp executable.");
        for (const candidate of candidates) {
            console.error(`Looked for: ${candidate}`);
        }
        console.error(
            "Build the server first with `cargo build -p rhai-lsp` or `cargo build --release -p rhai-lsp`, or set RHAI_SERVER_PATH.",
        );
        process.exit(1);
    }

    stageServerBinary(target, serverPath);
    return [target];
}

function normalizeManifestEntries(entries) {
    if (Array.isArray(entries)) {
        return entries.map((entry) => [entry.target, entry.path]);
    }

    if (entries && typeof entries === "object" && entries.targets) {
        return Object.entries(entries.targets);
    }

    if (entries && typeof entries === "object") {
        return Object.entries(entries);
    }

    throw new Error("Unsupported server manifest format.");
}

function stageServerBinary(target, source) {
    const destinationDir = path.join(serverDir, target);
    fs.mkdirSync(destinationDir, { recursive: true });
    fs.copyFileSync(source, path.join(destinationDir, executableNameForTarget(target)));
}

function currentVsCodeTarget() {
    switch (process.platform) {
        case "win32":
            switch (process.arch) {
                case "x64":
                    return "win32-x64";
                case "arm64":
                    return "win32-arm64";
                default:
                    return undefined;
            }

        case "darwin":
            switch (process.arch) {
                case "x64":
                    return "darwin-x64";
                case "arm64":
                    return "darwin-arm64";
                default:
                    return undefined;
            }

        case "linux":
            switch (process.arch) {
                case "x64":
                    return familySync() === MUSL ? "alpine-x64" : "linux-x64";
                case "arm64":
                    return familySync() === MUSL ? "alpine-arm64" : "linux-arm64";
                case "arm":
                    return "linux-armhf";
                default:
                    return undefined;
            }

        default:
            return undefined;
    }
}

function executableNameForTarget(target) {
    return target.startsWith("win32-") ? "rhai-lsp.exe" : "rhai-lsp";
}
