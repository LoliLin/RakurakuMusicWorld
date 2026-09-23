<#
.SYNOPSIS
    RakurakuMusicWorld - Complete Multi-Platform Build Script
.DESCRIPTION
    Builds all components of the RakurakuMusicWorld system:
    1. Web Frontend & Rust Backend Server (dist/)
    2. Windows Desktop Native App (dist-desktop/)
    3. Android Mobile APK (dist-android/)
.EXAMPLE
    .\build_all.ps1
#>

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $ScriptDir

Write-Host "==============================================" -ForegroundColor Green
Write-Host "   RakurakuMusicWorld - Full Multi-Platform Build" -ForegroundColor Green
Write-Host "==============================================" -ForegroundColor Green
Write-Host ""

# 1. Server & Web Package (dist/)
Write-Host "[1/3] Building Backend Services & Web Distribution (dist/)..." -ForegroundColor Cyan
& (Join-Path $ScriptDir "build_release.ps1")

# 2. Windows Desktop Client (dist-desktop/)
Write-Host ""
Write-Host "[2/3] Building Windows Desktop Client (dist-desktop/)..." -ForegroundColor Cyan
Push-Location (Join-Path $ScriptDir "radio-backend\frontend")
try {
    node scripts\build-desktop.mjs
} finally {
    Pop-Location
}

# 3. Android Mobile APK (dist-android/)
Write-Host ""
Write-Host "[3/3] Building Android Mobile App (dist-android/)..." -ForegroundColor Cyan
& (Join-Path $ScriptDir "build_android.ps1")

Write-Host ""
Write-Host "==============================================" -ForegroundColor Green
Write-Host "   All Multi-Platform Builds Complete!" -ForegroundColor Green
Write-Host "==============================================" -ForegroundColor Green
Write-Host ""
Write-Host "Artifacts:"
Write-Host "  1. Server / Web:   $ScriptDir\dist\"
Write-Host "  2. Desktop App:    $ScriptDir\dist-desktop\RakurakuMusicWorld-Portable-2.0.0.exe"
Write-Host "  3. Android App:    $ScriptDir\dist-android\RakurakuMusicWorld-debug.apk"
Write-Host ""
