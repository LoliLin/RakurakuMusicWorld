<#
.SYNOPSIS
    RakurakuMusicWorld - Windows PowerShell Release Build Script
.DESCRIPTION
    Builds the frontend (React 19 + Vite) and Rust backend (radio-backend & rakuraku-music-world-server),
    and packages a complete standalone distribution into the dist/ directory.
.PARAMETER SkipFrontend
    Skip frontend build and compile only the Rust backend.
.EXAMPLE
    .\build_release.ps1
    .\build_release.ps1 -SkipFrontend
#>

param (
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$BackendDir = Join-Path $ScriptDir "radio-backend"
$FrontendDir = Join-Path $BackendDir "frontend"
$DistDir = Join-Path $ScriptDir "dist"
$TargetDir = Join-Path $BackendDir "target"

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "    RakurakuMusicWorld Release Build (Windows)" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""

# 1. Environment check
Write-Host "[*] Checking environment..." -ForegroundColor Cyan

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[!] cargo not found on PATH. Please install Rust toolchain." -ForegroundColor Red
    exit 1
} else {
    Write-Host "[✓] Rust: $(rustc --version)" -ForegroundColor Green
}

foreach ($cmd in @("ffmpeg", "ffprobe")) {
    if (Get-Command $cmd -ErrorAction SilentlyContinue) {
        Write-Host "[✓] Found $cmd" -ForegroundColor Green
    } else {
        Write-Host "[!] $cmd not found on PATH (required at runtime for audio playback/scanning)" -ForegroundColor Yellow
    }
}

# 2. Frontend build
if (-not $SkipFrontend) {
    Write-Host "[*] Building frontend (React 19 + Vite)..." -ForegroundColor Cyan
    Push-Location $FrontendDir
    try {
        if (-not (Test-Path "node_modules")) {
            Write-Host "[*] Installing npm dependencies..." -ForegroundColor Cyan
            npm install
        }
        npm run build
        Write-Host "[✓] Frontend built to radio-backend/static" -ForegroundColor Green
    } finally {
        Pop-Location
    }
} else {
    Write-Host "[!] Skipping frontend build (-SkipFrontend)" -ForegroundColor Yellow
}

# 3. Rust release build
Write-Host "[*] Building Rust backend (--release)..." -ForegroundColor Cyan
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-msvc'
Push-Location $BackendDir
try {
    cargo build --release
    Write-Host "[✓] Rust release build complete" -ForegroundColor Green
} finally {
    Pop-Location
}

# 4. Prepare dist/ directory
Write-Host "[*] Preparing dist/ distribution package..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $DistDir "data") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $DistDir "media") | Out-Null

$ReleaseTarget = Join-Path $TargetDir "release"

# Copy radio-backend.exe
$RadioExe = Join-Path $ReleaseTarget "radio-backend.exe"
if (Test-Path $RadioExe) {
    Copy-Item -Force $RadioExe (Join-Path $DistDir "radio-backend.exe")
    Write-Host "[✓] Copied radio-backend.exe" -ForegroundColor Green
}

# Copy rakuraku-music-world-server.exe
$ServerExe = Join-Path $ReleaseTarget "rakuraku-music-world-server.exe"
if (Test-Path $ServerExe) {
    Copy-Item -Force $ServerExe (Join-Path $DistDir "rakuraku-music-world-server.exe")
    Write-Host "[✓] Copied rakuraku-music-world-server.exe (Dedicated Server)" -ForegroundColor Green
}

# Copy static/
$StaticSrc = Join-Path $BackendDir "static"
$StaticDest = Join-Path $DistDir "static"
if (Test-Path $StaticSrc) {
    if (Test-Path $StaticDest) {
        Remove-Item -Recurse -Force $StaticDest
    }
    Copy-Item -Recurse -Force $StaticSrc $StaticDest
    Write-Host "[✓] Copied static/ frontend assets" -ForegroundColor Green
}

# Copy config.toml (if missing)
$ConfigExample = Join-Path $BackendDir "config.toml.example"
$DistConfig = Join-Path $DistDir "config.toml"
if (-not (Test-Path $DistConfig)) {
    Copy-Item -Force $ConfigExample $DistConfig
    Write-Host "[✓] Created dist/config.toml from example" -ForegroundColor Green
} else {
    Write-Host "[✓] Preserved existing dist/config.toml" -ForegroundColor Green
}

# 5. Generate Windows batch & PowerShell launchers
# start.bat
@'
@echo off
cd /d "%~dp0"
echo Starting RakurakuMusicWorld...
start "" "%~dp0radio-backend.exe"
echo Server started in background.
echo Web UI: http://localhost:2241
'@ | Set-Content -Path (Join-Path $DistDir "start.bat") -Encoding ASCII

# start.ps1
@'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir
Write-Host "Starting RakurakuMusicWorld..." -ForegroundColor Cyan
Start-Process -FilePath (Join-Path $ScriptDir "radio-backend.exe") -WorkingDirectory $ScriptDir
Write-Host "Server started! Visit http://localhost:2241" -ForegroundColor Green
'@ | Set-Content -Path (Join-Path $DistDir "start.ps1") -Encoding UTF8

# start-server.bat
@'
@echo off
cd /d "%~dp0"
echo Starting RakurakuMusicWorld Dedicated Server (Headless)...
start "" "%~dp0rakuraku-music-world-server.exe"
echo Dedicated Server started on port 2241.
'@ | Set-Content -Path (Join-Path $DistDir "start-server.bat") -Encoding ASCII

# start-server.ps1
@'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir
Write-Host "Starting RakurakuMusicWorld Dedicated Server (Headless)..." -ForegroundColor Cyan
Start-Process -FilePath (Join-Path $ScriptDir "rakuraku-music-world-server.exe") -WorkingDirectory $ScriptDir
Write-Host "Dedicated server started on port 2241!" -ForegroundColor Green
'@ | Set-Content -Path (Join-Path $DistDir "start-server.ps1") -Encoding UTF8

# stop.bat
@'
@echo off
echo Stopping RakurakuMusicWorld...
taskkill /F /IM radio-backend.exe 2>nul
taskkill /F /IM rakuraku-music-world-server.exe 2>nul
echo Done.
'@ | Set-Content -Path (Join-Path $DistDir "stop.bat") -Encoding ASCII

# stop.ps1
@'
Write-Host "Stopping RakurakuMusicWorld..." -ForegroundColor Cyan
Get-Process -Name "radio-backend", "rakuraku-music-world-server" -ErrorAction SilentlyContinue | Stop-Process -Force
Write-Host "Servers stopped." -ForegroundColor Green
'@ | Set-Content -Path (Join-Path $DistDir "stop.ps1") -Encoding UTF8

Write-Host ""
Write-Host "==============================================" -ForegroundColor Green
Write-Host "    Build & Packaging Complete!" -ForegroundColor Green
Write-Host "==============================================" -ForegroundColor Green
Write-Host "Output Directory: $DistDir"
Write-Host ""
Write-Host "Quick Start:"
Write-Host "  cd dist"
Write-Host "  .\start.bat     # or .\start.ps1"
Write-Host "  Visit: http://localhost:2241"
Write-Host ""
