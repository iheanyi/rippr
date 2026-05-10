$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$BinariesDir = Join-Path $ProjectRoot "src-tauri\binaries"
$BuildVariant = "lgpl"

switch ($env:PROCESSOR_ARCHITECTURE) {
  "AMD64" {
    $TargetTriple = "x86_64-pc-windows-msvc"
    $BuildFlavor = "win64"
  }
  "ARM64" {
    $TargetTriple = "aarch64-pc-windows-msvc"
    $BuildFlavor = "winarm64"
  }
  default {
    throw "Unsupported Windows architecture: $env:PROCESSOR_ARCHITECTURE"
  }
}

$FfmpegUrl = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-$BuildFlavor-$BuildVariant.zip"
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "rippr-ffmpeg-$BuildFlavor"
$TempZip = "$TempRoot.zip"
$FfmpegDestination = Join-Path $BinariesDir "ffmpeg-$TargetTriple.exe"
$FfprobeDestination = Join-Path $BinariesDir "ffprobe-$TargetTriple.exe"
$VariantMarker = Join-Path $BinariesDir "ffmpeg-$TargetTriple.variant"
$ForceDownload = $env:RIPPR_FORCE_FFMPEG -eq "1"

New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null

if (Test-Path -LiteralPath $VariantMarker) {
  $InstalledVariant = Get-Content -LiteralPath $VariantMarker -First 1
}
else {
  $InstalledVariant = $null
}

if ((-not $ForceDownload) -and ($InstalledVariant -eq $BuildVariant) -and (Test-Path -LiteralPath $FfmpegDestination) -and (Test-Path -LiteralPath $FfprobeDestination)) {
  Write-Host "ffmpeg already exists for $TargetTriple ($BuildVariant)."
  Write-Host "Set RIPPR_FORCE_FFMPEG=1 to download it again."
  Get-ChildItem -LiteralPath $BinariesDir
  exit 0
}

Write-Host "Downloading ffmpeg for $TargetTriple ($BuildVariant)..."
Write-Host "URL: $FfmpegUrl"

if (Test-Path -LiteralPath $TempRoot) {
  Remove-Item -LiteralPath $TempRoot -Recurse -Force
}

try {
  Invoke-WebRequest -Uri $FfmpegUrl -OutFile $TempZip
  Expand-Archive -LiteralPath $TempZip -DestinationPath $TempRoot -Force

  $FfmpegExe = Get-ChildItem -LiteralPath $TempRoot -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
  $FfprobeExe = Get-ChildItem -LiteralPath $TempRoot -Recurse -Filter "ffprobe.exe" | Select-Object -First 1

  if (-not $FfmpegExe) {
    throw "ffmpeg.exe not found in downloaded archive"
  }
  if (-not $FfprobeExe) {
    throw "ffprobe.exe not found in downloaded archive"
  }

  Copy-Item -LiteralPath $FfmpegExe.FullName -Destination $FfmpegDestination -Force
  Copy-Item -LiteralPath $FfprobeExe.FullName -Destination $FfprobeDestination -Force
  Set-Content -LiteralPath $VariantMarker -Value $BuildVariant -Encoding ASCII
}
finally {
  if (Test-Path -LiteralPath $TempZip) {
    Remove-Item -LiteralPath $TempZip -Force
  }
  if (Test-Path -LiteralPath $TempRoot) {
    Remove-Item -LiteralPath $TempRoot -Recurse -Force
  }
}

Write-Host ""
Write-Host "ffmpeg binaries installed to $BinariesDir"
Get-ChildItem -LiteralPath $BinariesDir
