# Builds the portable exe (no installer) and copies it next to the installers,
# into <target dir>/release/bundle/portable/. Used by `npm run build` and CI.
param(
    # Rust target triple, e.g. x86_64-pc-windows-msvc. Omit to build for the host.
    [string]$Target
)

$root = Split-Path $PSScriptRoot -Parent
$conf = Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw -ErrorAction Stop | ConvertFrom-Json

$tauriArgs = @("tauri", "build", "--no-bundle", "--features", "portable")
if ($Target) { $tauriArgs += @("--target", $Target) }

Push-Location $root
& npx @tauriArgs
$code = $LASTEXITCODE
Pop-Location
if ($code -ne 0) { exit $code }

$releaseDir = if ($Target) {
    Join-Path $root "src-tauri\target\$Target\release"
} else {
    Join-Path $root "src-tauri\target\release"
}
$outDir = Join-Path $releaseDir "bundle\portable"
New-Item -ItemType Directory -Force -Path $outDir -ErrorAction Stop | Out-Null

$name = "$($conf.productName -replace ' ', '.')_$($conf.version)_x64-portable.exe"
$exe = Join-Path $outDir $name
Copy-Item (Join-Path $releaseDir "zen-sync.exe") $exe -Force -ErrorAction Stop
Write-Host "Portable exe: $exe"
