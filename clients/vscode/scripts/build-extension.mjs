import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { build, context } from "esbuild";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientRoot = path.resolve(scriptDir, "..");
const outDir = path.join(clientRoot, "out");
const watchMode = process.argv.includes("--watch");

const buildOptions = {
    entryPoints: [path.join(clientRoot, "src", "extension.ts")],
    outfile: path.join(outDir, "extension.js"),
    bundle: true,
    platform: "node",
    format: "cjs",
    target: ["node20"],
    external: ["vscode"],
    sourcemap: true,
    logLevel: "info",
    legalComments: "none",
};

fs.rmSync(outDir, { recursive: true, force: true });

if (watchMode) {
    const ctx = await context(buildOptions);
    await ctx.watch();
    console.log("Watching VSCode extension sources with esbuild...");
} else {
    await build(buildOptions);
}
