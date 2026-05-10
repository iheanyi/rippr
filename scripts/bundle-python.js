import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.dirname(scriptDir);

const command = process.platform === "win32" ? "powershell.exe" : "bash";
const args =
  process.platform === "win32"
    ? [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        path.join(scriptDir, "bundle-python.ps1"),
      ]
    : [path.join(scriptDir, "bundle-python.sh")];

const result = spawnSync(command, args, {
  cwd: projectRoot,
  stdio: "inherit",
  shell: false,
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
