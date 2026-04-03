import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientRoot = path.resolve(scriptDir, "..");

for (const relativePath of [".artifacts", ".staging", "out", "server"]) {
    fs.rmSync(path.join(clientRoot, relativePath), {
        recursive: true,
        force: true,
    });
}

console.log("Cleaned VSCode extension build artifacts.");
