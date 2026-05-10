import { runPlatformScript } from "./run-platform-script.js";

runPlatformScript("package-portable", {
  windowsOnly: true,
  skipMessage: "Skipping Windows portable package on non-Windows platform.",
});
