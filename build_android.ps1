<#
.SYNOPSIS
    RakurakuMusicWorld - Android APK Build Script (Capacitor)
.DESCRIPTION
    Builds the React 19 frontend, syncs web assets into the Android native project,
    and invokes Gradle to compile the Android debug APK package.
.EXAMPLE
    .\build_android.ps1
#>

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$FrontendDir = Join-Path $ScriptDir "radio-backend\frontend"
$AndroidDir = Join-Path $FrontendDir "android"
$DistAndroidDir = Join-Path $ScriptDir "dist-android"

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "    RakurakuMusicWorld Android Build (Capacitor)" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
Write-Host ""

# 1. Environment check
Write-Host "[*] Checking environment..." -ForegroundColor Cyan

# Check Java 21
$JavaHome = "D:\Program Disk\Java\azul-21.0.5"
if (Test-Path $JavaHome) {
    $env:JAVA_HOME = $JavaHome
    $env:Path = "$JavaHome\bin;" + $env:Path
    Write-Host "[✓] Using Java 21 LTS: $JavaHome" -ForegroundColor Green
} else {
    Write-Host "[!] Java 21 not found at default path, using current java..." -ForegroundColor Yellow
}

# Check Android SDK
$SdkDir = "$env:LOCALAPPDATA\Android\Sdk"
if (Test-Path $SdkDir) {
    $env:ANDROID_HOME = $SdkDir
    Write-Host "[✓] Android SDK: $SdkDir" -ForegroundColor Green
} else {
    Write-Host "[!] Android SDK not found at $SdkDir" -ForegroundColor Yellow
}

# 2. Build Frontend
Write-Host "[*] Building React 19 frontend..." -ForegroundColor Cyan
Push-Location $FrontendDir
try {
    npm run build
    Write-Host "[✓] Frontend built." -ForegroundColor Green
    
    Write-Host "[*] Syncing web assets with Capacitor..." -ForegroundColor Cyan
    npx cap sync android
    Write-Host "[✓] Capacitor synced." -ForegroundColor Green
} finally {
    Pop-Location
}

# 3. Build APK with Gradle
Write-Host "[*] Compiling Android APK with Gradle..." -ForegroundColor Cyan
Push-Location $AndroidDir
try {
    cmd.exe /c gradlew.bat assembleDebug
    Write-Host "[✓] Gradle assembleDebug succeeded!" -ForegroundColor Green
} finally {
    Pop-Location
}

# 4. Copy output APK
New-Item -ItemType Directory -Force -Path $DistAndroidDir | Out-Null
$ApkSrc = Join-Path $AndroidDir "app\build\outputs\apk\debug\app-debug.apk"
$ApkDest = Join-Path $DistAndroidDir "RakurakuMusicWorld-debug.apk"

if (Test-Path $ApkSrc) {
    Copy-Item -Force $ApkSrc $ApkDest
    Write-Host ""
    Write-Host "==============================================" -ForegroundColor Green
    Write-Host "    Android APK Build Complete!" -ForegroundColor Green
    Write-Host "==============================================" -ForegroundColor Green
    Write-Host "APK Location: $ApkDest" -ForegroundColor Cyan
    Write-Host "Install to connected phone: adb install -r `"$ApkDest`""
} else {
    Write-Host "[✗] Could not find built APK at $ApkSrc" -ForegroundColor Red
    exit 1
}
