import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const artifactSuffix = process.argv[2] || process.env.RIPPR_RELEASE_SUFFIX || "";
const targetRoot = path.join(root, "src-tauri", "target");
const outputDir = path.join(root, "release-assets");
const allowedSuffixes = [
  ".appimage",
  ".deb",
  ".dmg",
  ".exe",
  ".msi",
  ".rpm",
  ".sig",
  ".tar.gz",
  ".tar.gz.sig",
  ".zip",
];

function fail(message) {
  console.error(message);
  process.exit(1);
}

function walk(dir) {
  if (!fs.existsSync(dir)) {
    return [];
  }

  const entries = fs.readdirSync(dir, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      return walk(fullPath);
    }
    return [fullPath];
  });
}

function isTopLevelBundleOutput(filePath) {
  const parts = filePath.split(path.sep);
  const bundleIndex = parts.lastIndexOf("bundle");

  if (bundleIndex === -1) {
    return false;
  }

  const relativeBundleParts = parts.slice(bundleIndex + 2);
  return relativeBundleParts.length === 1;
}

function isReleaseAsset(filePath) {
  const lowerName = path.basename(filePath).toLowerCase();
  return allowedSuffixes.some((suffix) => lowerName.endsWith(suffix)) && isTopLevelBundleOutput(filePath);
}

function splitAssetName(fileName) {
  const lowerName = fileName.toLowerCase();
  for (const compoundSuffix of [".tar.gz.sig", ".tar.gz"]) {
    if (lowerName.endsWith(compoundSuffix)) {
      return [fileName.slice(0, -compoundSuffix.length), fileName.slice(-compoundSuffix.length)];
    }
  }

  const extension = path.extname(fileName);
  return [fileName.slice(0, -extension.length), extension];
}

function stagedName(filePath) {
  const fileName = path.basename(filePath);
  const [stem, extension] = splitAssetName(fileName);

  if (!artifactSuffix) {
    return fileName;
  }

  return `${stem}-${artifactSuffix}${extension}`;
}

fs.rmSync(outputDir, { force: true, recursive: true });
fs.mkdirSync(outputDir, { recursive: true });

const assets = walk(targetRoot).filter(isReleaseAsset).sort();

if (assets.length === 0) {
  fail(`No release assets found under ${targetRoot}.`);
}

const stagedAssets = [];

for (const asset of assets) {
  const destination = path.join(outputDir, stagedName(asset));
  fs.copyFileSync(asset, destination);
  stagedAssets.push(destination);
}

console.log("Staged release assets:");
for (const asset of stagedAssets) {
  console.log(`- ${path.relative(root, asset)}`);
}
