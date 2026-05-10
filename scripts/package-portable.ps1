$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$PackageJson = Get-Content -Raw -LiteralPath (Join-Path $ProjectRoot "package.json") | ConvertFrom-Json
$Version = $PackageJson.version
$Arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }
$TargetTriple = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }
$TauriDir = Join-Path $ProjectRoot "src-tauri"
$ReleaseDir = Join-Path $TauriDir "target\release"
$BinariesDir = Join-Path $TauriDir "binaries"
$SourceResourcesDir = Join-Path $TauriDir "resources"
$PortableRoot = Join-Path $ReleaseDir "bundle\portable"
$PortableDir = Join-Path $PortableRoot "Rippr_$Version`_$Arch-portable"
$ZipPath = "$PortableDir.zip"

function Select-FirstExistingPath {
  param(
    [Parameter(Mandatory = $true)]
    [string[]] $Paths,
    [Parameter(Mandatory = $true)]
    [string] $MissingMessage
  )

  foreach ($Candidate in $Paths) {
    if (Test-Path -LiteralPath $Candidate) {
      return $Candidate
    }
  }

  throw $MissingMessage
}

function Select-ResourcesPath {
  param(
    [Parameter(Mandatory = $true)]
    [string[]] $Paths
  )

  foreach ($Candidate in $Paths) {
    if ((Test-Path -LiteralPath $Candidate) -and (Test-Path -LiteralPath (Join-Path $Candidate "python"))) {
      return $Candidate
    }
  }

  throw "Missing bundled Python resources. Run npm run bundle-python before packaging the portable build."
}

$AppExe = Select-FirstExistingPath `
  -Paths @((Join-Path $ReleaseDir "rippr.exe")) `
  -MissingMessage "Missing Rippr release binary. Run npm run build:fast before packaging the portable build."

$FfmpegExe = Select-FirstExistingPath `
  -Paths @(
    (Join-Path $ReleaseDir "ffmpeg.exe"),
    (Join-Path $BinariesDir "ffmpeg-$TargetTriple.exe")
  ) `
  -MissingMessage "Missing bundled ffmpeg. Run npm run bundle-ffmpeg before packaging the portable build."

$FfprobeExe = Select-FirstExistingPath `
  -Paths @(
    (Join-Path $ReleaseDir "ffprobe.exe"),
    (Join-Path $BinariesDir "ffprobe-$TargetTriple.exe")
  ) `
  -MissingMessage "Missing bundled ffprobe. Run npm run bundle-ffmpeg before packaging the portable build."

$ResourcesDir = Select-ResourcesPath `
  -Paths @(
    (Join-Path $ReleaseDir "resources"),
    $SourceResourcesDir
  )

if (Test-Path -LiteralPath $PortableDir) {
  Remove-Item -LiteralPath $PortableDir -Recurse -Force
}
if (Test-Path -LiteralPath $ZipPath) {
  Remove-Item -LiteralPath $ZipPath -Force
}

New-Item -ItemType Directory -Force -Path $PortableDir | Out-Null

Copy-Item -LiteralPath $AppExe -Destination (Join-Path $PortableDir "rippr.exe")
Copy-Item -LiteralPath $FfmpegExe -Destination (Join-Path $PortableDir "ffmpeg.exe")
Copy-Item -LiteralPath $FfprobeExe -Destination (Join-Path $PortableDir "ffprobe.exe")
Copy-Item -LiteralPath $ResourcesDir -Destination (Join-Path $PortableDir "resources") -Recurse

$ReadmePath = Join-Path $PortableDir "README-PORTABLE.txt"
@"
Rippr portable build

Run rippr.exe from this folder.

This portable package includes:
- Rippr
- ffmpeg and ffprobe
- standalone Python with yt-dlp

No installer is required. The app may still use your Windows user profile for settings, download history, and WebView2 runtime data.
"@ | Set-Content -LiteralPath $ReadmePath -Encoding UTF8

Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory(
  $PortableDir,
  $ZipPath,
  [System.IO.Compression.CompressionLevel]::Fastest,
  $false
)

Write-Host "Portable package created:"
Write-Host $ZipPath
