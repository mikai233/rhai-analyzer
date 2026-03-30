import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");
const cargoTomlPath = path.join(repoRoot, "Cargo.toml");
const vscodePackagePath = path.join(repoRoot, "clients", "vscode", "package.json");
const vscodeLockPath = path.join(repoRoot, "clients", "vscode", "package-lock.json");

const args = parseArgs(process.argv.slice(2));
const current = readVersions();

if (!allEqual(current)) {
    if (!args.allowMismatch) {
        throw new Error(
            `Version mismatch detected. Cargo.toml=${current.cargo}, clients/vscode/package.json=${current.packageJson}, clients/vscode/package-lock.json=${current.packageLock}`,
        );
    }
}

const mode = args.mode ?? "none";
const newVersion = computeVersion(current.cargo, mode, args.version);
const changed = newVersion !== current.cargo || !allEqual(current);

if (args.write && changed) {
    writeCargoVersion(newVersion);
    writePackageVersion(vscodePackagePath, newVersion);
    writeLockVersion(newVersion);
}

const result = {
    currentVersion: current.cargo,
    newVersion,
    changed,
    mode,
};

if (args.json) {
    process.stdout.write(`${JSON.stringify(result)}\n`);
} else {
    process.stdout.write(`${newVersion}\n`);
}

function parseArgs(argv) {
    const parsed = {
        write: false,
        json: false,
    };

    for (let index = 0; index < argv.length; index += 1) {
        const arg = argv[index];
        switch (arg) {
            case "--mode":
                parsed.mode = argv[++index];
                break;
            case "--version":
                parsed.version = argv[++index];
                break;
            case "--write":
                parsed.write = true;
                break;
            case "--json":
                parsed.json = true;
                break;
            case "--allow-mismatch":
                parsed.allowMismatch = true;
                break;
            default:
                throw new Error(`Unknown argument: ${arg}`);
        }
    }

    return parsed;
}

function readVersions() {
    const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
    const packageJson = JSON.parse(fs.readFileSync(vscodePackagePath, "utf8"));
    const packageLock = JSON.parse(fs.readFileSync(vscodeLockPath, "utf8"));

    return {
        cargo: extractCargoVersion(cargoToml),
        packageJson: packageJson.version,
        packageLock: packageLock.version ?? packageLock.packages?.[""]?.version,
    };
}

function allEqual(versions) {
    return versions.cargo === versions.packageJson
        && versions.packageJson === versions.packageLock;
}

function extractCargoVersion(contents) {
    const match = contents.match(/(^version\s*=\s*")(\d+\.\d+\.\d+)(")/m);
    if (!match) {
        throw new Error("Could not locate workspace.package.version in Cargo.toml.");
    }

    return match[2];
}

function computeVersion(currentVersion, mode, exactVersion) {
    switch (mode) {
        case "none":
            return currentVersion;
        case "exact":
            if (!exactVersion) {
                throw new Error("`--version` is required when `--mode exact` is used.");
            }
            assertSemver(exactVersion);
            return exactVersion;
        case "major":
        case "minor":
        case "patch":
            return bumpVersion(currentVersion, mode);
        default:
            throw new Error(`Unsupported version mode: ${mode}`);
    }
}

function bumpVersion(version, mode) {
    const [major, minor, patch] = parseSemver(version);

    switch (mode) {
        case "major":
            return `${major + 1}.0.0`;
        case "minor":
            return `${major}.${minor + 1}.0`;
        case "patch":
            return `${major}.${minor}.${patch + 1}`;
        default:
            throw new Error(`Unsupported bump mode: ${mode}`);
    }
}

function parseSemver(version) {
    assertSemver(version);
    return version.split(".").map((part) => Number.parseInt(part, 10));
}

function assertSemver(version) {
    if (!/^\d+\.\d+\.\d+$/.test(version)) {
        throw new Error(
            `Expected a stable semver version like 1.2.3, received ${version}.`,
        );
    }
}

function writeCargoVersion(version) {
    const currentContents = fs.readFileSync(cargoTomlPath, "utf8");
    const nextContents = currentContents.replace(
        /(^version\s*=\s*")(\d+\.\d+\.\d+)(")/m,
        `$1${version}$3`,
    );

    fs.writeFileSync(cargoTomlPath, nextContents);
}

function writePackageVersion(packagePath, version) {
    const packageJson = JSON.parse(fs.readFileSync(packagePath, "utf8"));
    packageJson.version = version;
    fs.writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);
}

function writeLockVersion(version) {
    const packageLock = JSON.parse(fs.readFileSync(vscodeLockPath, "utf8"));
    packageLock.version = version;
    if (packageLock.packages?.[""]) {
        packageLock.packages[""].version = version;
    }
    fs.writeFileSync(vscodeLockPath, `${JSON.stringify(packageLock, null, 2)}\n`);
}
