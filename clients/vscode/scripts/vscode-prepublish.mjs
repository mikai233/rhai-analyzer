import { spawnSync } from "node:child_process";

if (process.env.RHAI_SKIP_VSCODE_PREPUBLISH === "1") {
    console.log("Skipping vscode:prepublish because the extension was already built.");
    process.exit(0);
}

const result = spawnSync("npm", ["run", "build"], {
    stdio: "inherit",
    shell: process.platform === "win32",
});

if (result.status !== 0) {
    process.exit(result.status ?? 1);
}
