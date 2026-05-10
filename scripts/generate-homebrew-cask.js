import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

function parseArgs(args) {
  const parsed = {};

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (!arg.startsWith("--")) {
      continue;
    }

    const key = arg.slice(2);
    const value = args[index + 1];
    if (!value || value.startsWith("--")) {
      parsed[key] = "true";
      continue;
    }

    parsed[key] = value;
    index += 1;
  }

  return parsed;
}

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

function sha256(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function encodeAssetName(fileName) {
  return encodeURIComponent(fileName).replace(/[!'()*]/g, (character) =>
    `%${character.charCodeAt(0).toString(16).toUpperCase()}`,
  );
}

function makeUrl(repo, tag, fileName) {
  return `https://github.com/${repo}/releases/download/${tag}/${encodeAssetName(fileName)}`;
}

const args = parseArgs(process.argv.slice(2));
const artifactsDir = path.resolve(process.cwd(), args.artifacts || "release-assets");
const version = args.version;
const tag = args.tag || (version ? `v${version}` : "");
const repo = args.repo || "iheanyi/rippr";
const output = path.resolve(process.cwd(), args.output || path.join("dist", "homebrew", "rippr.rb"));

if (!version) {
  fail("Missing --version.");
}

if (!tag) {
  fail("Missing --tag.");
}

const dmgs = walk(artifactsDir)
  .filter((filePath) => filePath.toLowerCase().endsWith(".dmg"))
  .map((filePath) => ({
    filePath,
    name: path.basename(filePath),
  }));

const armDmg = dmgs.find((asset) => /(?:aarch64|arm64|macos-arm64)/i.test(asset.name));
const intelDmg = dmgs.find((asset) => /(?:x64|x86_64|intel|macos-x64)/i.test(asset.name));

if (!armDmg || !intelDmg) {
  fail("Homebrew cask generation requires both Apple Silicon and Intel macOS DMG assets.");
}

const cask = `cask "rippr" do
  version "${version}"

  on_arm do
    sha256 "${sha256(armDmg.filePath)}"
    url "${makeUrl(repo, tag, armDmg.name)}"
  end

  on_intel do
    sha256 "${sha256(intelDmg.filePath)}"
    url "${makeUrl(repo, tag, intelDmg.name)}"
  end

  name "Rippr"
  desc "Desktop app for downloading audio samples"
  homepage "https://github.com/${repo}"

  app "Rippr.app"
end
`;

fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, cask, "utf8");

console.log(`Generated Homebrew cask at ${output}`);
