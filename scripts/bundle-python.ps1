$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$ResourcesDir = Join-Path $ProjectRoot "src-tauri\resources"
$PythonVersion = "3.13.1"
$ReleaseDate = "20250115"

switch ($env:PROCESSOR_ARCHITECTURE) {
  "AMD64" {
    $Platform = "x86_64-pc-windows-msvc"
  }
  "ARM64" {
    $Platform = "aarch64-pc-windows-msvc"
  }
  default {
    throw "Unsupported Windows architecture: $env:PROCESSOR_ARCHITECTURE"
  }
}

$PythonUrl = "https://github.com/indygreg/python-build-standalone/releases/download/$ReleaseDate/cpython-$PythonVersion+$ReleaseDate-$Platform-install_only.tar.gz"
$PythonDir = Join-Path $ResourcesDir "python"
$PythonExe = Join-Path $PythonDir "python.exe"
$UpdateYtDlp = $env:RIPPR_UPDATE_YTDLP -eq "1"

Write-Host "Platform: $Platform"
Write-Host "Python URL: $PythonUrl"

New-Item -ItemType Directory -Force -Path $ResourcesDir | Out-Null

if (-not (Test-Path -LiteralPath $PythonDir)) {
  Write-Host "Downloading standalone Python..."
  $TempTar = Join-Path ([System.IO.Path]::GetTempPath()) "rippr-python-$Platform.tar.gz"

  try {
    Invoke-WebRequest -Uri $PythonUrl -OutFile $TempTar

    Write-Host "Extracting Python..."
    tar -xzf $TempTar -C $ResourcesDir
  }
  finally {
    if (Test-Path -LiteralPath $TempTar) {
      Remove-Item -LiteralPath $TempTar -Force
    }
  }
}
else {
  Write-Host "Python already exists at $PythonDir"
}

if (-not (Test-Path -LiteralPath $PythonExe)) {
  throw "Python binary not found at $PythonExe"
}

if (-not $UpdateYtDlp) {
  $YtDlpVersion = & $PythonExe -c "import yt_dlp; print(yt_dlp.version.__version__)" 2>$null

  if ($LASTEXITCODE -eq 0 -and $YtDlpVersion) {
    Write-Host "yt-dlp already installed: $YtDlpVersion"
    Write-Host "Set RIPPR_UPDATE_YTDLP=1 to update it."
    Write-Host ""
    Write-Host "Python bundling complete!"
    Write-Host "Bundled Python location: $PythonDir"
    exit 0
  }
}

Write-Host "Installing yt-dlp..."
& $PythonExe -m pip install --upgrade pip
& $PythonExe -m pip install --upgrade yt-dlp

Write-Host "Verifying yt-dlp installation..."
& $PythonExe -c "import yt_dlp; print(f'yt-dlp version: {yt_dlp.version.__version__}')"

Write-Host ""
Write-Host "Python bundling complete!"
Write-Host "Bundled Python location: $PythonDir"
