import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.dirname(scriptDir);

export function runPlatformScript(
  scriptName,
  { windowsOnly = false, skipMessage } = {},
) {
  if (windowsOnly && process.platform !== "win32") {
    console.log(skipMessage ?? `Skipping ${scriptName} on non-Windows platform.`);
    process.exit(0);
  }

  const command = process.platform === "win32" ? "powershell.exe" : "bash";
  const scriptPath = path.join(
    scriptDir,
    `${scriptName}.${process.platform === "win32" ? "ps1" : "sh"}`,
  );
  const args =
    process.platform === "win32"
      ? ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", scriptPath]
      : [scriptPath];

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
}
