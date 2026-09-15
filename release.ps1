param(
    [Parameter(Mandatory)]
    [string]$Version
)

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Error "Version must be in format X.Y.Z (e.g. 0.1.2)"
    exit 1
}

$tag = "v$Version"

# Check for uncommitted changes
if (git status --porcelain) {
    Write-Error "Working tree has uncommitted changes. Commit or stash them first."
    exit 1
}

$lockPath = "$PSScriptRoot\src-tauri\Cargo.lock"
$lock = Get-Content $lockPath -Raw
$lockPattern = '(\[\[package\]\]\r?\nname = "zen-sync"\r?\nversion = )"[^"]+"'
if ($lock -notmatch $lockPattern) {
    Write-Error "zen-sync entry not found in Cargo.lock"
    exit 1
}

# Bump tauri.conf.json (in-place regex, preserves formatting)
$confPath = "$PSScriptRoot\src-tauri\tauri.conf.json"
$conf = Get-Content $confPath -Raw
$conf = $conf -replace '("version":\s*)"[^"]+"', "`$1`"$Version`""
[IO.File]::WriteAllText($confPath, $conf)

# Bump Cargo.toml — only the [package] version (first occurrence before any [dep] section)
$cargoPath = "$PSScriptRoot\src-tauri\Cargo.toml"
$lines = Get-Content $cargoPath
$inPackage = $false
$replaced = $false
$lines = $lines | ForEach-Object {
    if ($_ -match '^\[') { $inPackage = ($_ -match '^\[package\]') }
    if ($inPackage -and -not $replaced -and $_ -match '^version\s*=') {
        $replaced = $true
        "version = `"$Version`""
    } else {
        $_
    }
}
[IO.File]::WriteAllLines($cargoPath, $lines)

# Bump the app's own entry in Cargo.lock so the next build doesn't leave it modified
$lock = $lock -replace $lockPattern, "`$1`"$Version`""
[IO.File]::WriteAllText($lockPath, $lock)

Write-Host "Bumped version to $Version"

git add src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: release $tag"
git tag $tag
git push origin main
git push origin $tag

Write-Host "Pushed $tag — GitHub Actions will build and publish the release."
