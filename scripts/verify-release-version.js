import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const tag = process.argv[2] || process.env.GITHUB_REF_NAME || "";
const semverTagPattern = /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

function fail(message) {
  console.error(message);
  process.exit(1);
}

function readJson(relativePath) {
  const fullPath = path.join(root, relativePath);
  return JSON.parse(fs.readFileSync(fullPath, "utf8"));
}

function readCargoVersion() {
  const cargoToml = fs.readFileSync(path.join(root, "src-tauri", "Cargo.toml"), "utf8");
  let inPackageSection = false;

  for (const line of cargoToml.split(/\r?\n/)) {
    if (/^\s*\[package\]\s*$/.test(line)) {
      inPackageSection = true;
      continue;
    }

    if (inPackageSection && /^\s*\[/.test(line)) {
      break;
    }

    if (inPackageSection) {
      const versionMatch = line.match(/^version\s*=\s*"([^"]+)"/);
      if (versionMatch) {
        return versionMatch[1];
      }
    }
  }

  fail("Could not find package.version in src-tauri/Cargo.toml.");
}

if (!semverTagPattern.test(tag)) {
  fail(`Release tag must look like v1.2.3 or v1.2.3-beta.1. Received: ${tag || "(empty)"}`);
}

const version = tag.slice(1);
const versions = {
  "package.json": readJson("package.json").version,
  "src-tauri/tauri.conf.json": readJson(path.join("src-tauri", "tauri.conf.json")).version,
  "src-tauri/Cargo.toml": readCargoVersion(),
};

const mismatches = Object.entries(versions).filter(([, current]) => current !== version);

if (mismatches.length > 0) {
  console.error(`Release tag ${tag} does not match the checked-in app versions:`);
  for (const [file, current] of Object.entries(versions)) {
    console.error(`- ${file}: ${current}`);
  }
  process.exit(1);
}

console.log(`Release version verified: ${version}`);
